/*
 * Copyright (c) 2006-Present, Redis Ltd.
 * All rights reserved.
 *
 * Licensed under your choice of the Redis Source Available License 2.0
 * (RSALv2); or (b) the Server Side Public License v1 (SSPLv1); or (c) the
 * GNU Affero General Public License v3 (AGPLv3).
*/

use std::{ffi::c_void, io::Cursor, sync::atomic};

use crate::{DecodedBy, Decoder, InvertedIndex, NumericDecoder, RSIndexResult, TermDecoder};
use ffi::{FieldSpec, IndexFlags, IndexFlags_Index_HasMultiValue, t_docId};

/// Filter details to apply to numeric values
/// cbindgen:rename-all=CamelCase
#[derive(Debug)]
#[repr(C)]
pub struct NumericFilter {
    /// The field specification which this filter is acting on
    pub field_spec: *const FieldSpec,

    /// Beginning of the range
    pub min: f64,

    /// End of the range
    pub max: f64,

    /// Geo filter, if any
    pub geo_filter: *const c_void,

    /// Range includes the min value
    pub min_inclusive: bool,

    /// Range includes the max value
    pub max_inclusive: bool,

    /// Order of SORTBY (ascending/descending)
    pub ascending: bool,

    /// Minimum number of results needed
    pub limit: usize,

    /// Number of results to skip
    pub offset: usize,
}

impl Default for NumericFilter {
    fn default() -> Self {
        Self {
            min: 0.0,
            max: f64::MAX,
            min_inclusive: true,
            max_inclusive: true,
            field_spec: std::ptr::null(),
            geo_filter: std::ptr::null(),
            ascending: true,
            limit: 0,
            offset: 0,
        }
    }
}

impl NumericFilter {
    /// Check if this is a numeric filter (and not a geo filter)
    pub const fn is_numeric_filter(&self) -> bool {
        self.geo_filter.is_null()
    }

    /// Check if the given value is in the range specified by this filter
    #[inline(always)]
    pub fn value_in_range(&self, value: f64) -> bool {
        let min_ok = value > self.min || (self.min_inclusive && value == self.min);
        let max_ok = value < self.max || (self.max_inclusive && value == self.max);

        min_ok && max_ok
    }
}

/// Reader that is able to read the records from an [`InvertedIndex`]
pub struct IndexReaderCore<'index, E> {
    /// The inverted index that is being read from.
    ii: &'index InvertedIndex<E>,

    /// The current position in the block that is being read from.
    current_buffer: Cursor<&'index [u8]>,

    /// The index of the current block in the `blocks` vector. This is used to keep track of
    /// which block we are currently reading from, especially when the current buffer is empty and we
    /// need to move to the next block.
    current_block_idx: usize,

    /// The last document ID that was read from the index. This is used to determine the base
    /// document ID for delta calculations.
    last_doc_id: t_docId,

    /// The marker of the inverted index when this reader last read from it. This is used to
    /// detect if the index has been modified since the last read, in which case the reader
    /// should be reset.
    gc_marker: u32,
}

/// A reader is something which knows how to read / decode the records from an `[InvertedIndex]`.
pub trait IndexReader<'index> {
    /// Read the next record from the index into `result`. If there are no more records to read,
    /// then `false` is returned.
    fn next_record(&mut self, result: &mut RSIndexResult<'index>) -> std::io::Result<bool>;

    /// Seek to the first record whose ID is higher or equal to the given document ID and put it
    /// on `recult`. If the end of the index is reached before finding the document ID, then `false`
    /// is returned.
    fn seek_record(
        &mut self,
        doc_id: t_docId,
        result: &mut RSIndexResult<'index>,
    ) -> std::io::Result<bool>;

    /// Skip forward to the block containing the given document ID. Returns false if the end of the
    /// index was reached and true otherwise.
    fn skip_to(&mut self, doc_id: t_docId) -> bool;

    /// Reset the reader to the beginning of the index.
    fn reset(&mut self);

    /// Return the number of unique documents in the underlying index.
    fn unique_docs(&self) -> u64;

    /// Returns true if the underlying index has duplicate document IDs.
    fn has_duplicates(&self) -> bool;

    /// Get the flags of the underlying index
    fn flags(&self) -> IndexFlags;

    /// Check if the underlying index has been modified since the last time this reader read from it.
    /// If it has, then the reader should be reset before reading from it again.
    fn needs_revalidation(&self) -> bool;

    /// Refresh buffer pointers in case blocks were reallocated without GC changes
    fn refresh_buffer_pointers(&mut self);
}

/// Marker trait for readers producing numeric values.
pub trait NumericReader<'index>: IndexReader<'index> {}

/// Marker trait for readers producing term values.
pub trait TermReader<'index>: IndexReader<'index> {}

// Automatically implemented if the IndexReaderCore uses a NumericDecoder.
impl<'index, E: DecodedBy<Decoder = D>, D: Decoder + NumericDecoder> NumericReader<'index>
    for IndexReaderCore<'index, E>
{
}

/// Automatically implemented if the IndexReaderCore uses a TermDecoder.
impl<'index, E: DecodedBy<Decoder = D>, D: Decoder + TermDecoder> TermReader<'index>
    for IndexReaderCore<'index, E>
{
}

impl<'index, E: DecodedBy<Decoder = D>, D: Decoder> IndexReader<'index>
    for IndexReaderCore<'index, E>
{
    #[inline(always)]
    fn next_record(&mut self, result: &mut RSIndexResult<'index>) -> std::io::Result<bool> {
        // Check if the current buffer is empty or the end of the buffer has been reached
        if self.current_buffer.get_ref().len() as u64 <= self.current_buffer.position() {
            if self.current_block_idx + 1 >= self.ii.blocks.len() {
                // No more blocks to read from
                return Ok(false);
            };

            self.set_current_block(self.current_block_idx + 1);
        }

        let base = D::base_id(&self.ii.blocks[self.current_block_idx], self.last_doc_id);
        D::decode(&mut self.current_buffer, base, result)?;

        self.last_doc_id = result.doc_id;

        Ok(true)
    }

    #[inline(always)]
    fn seek_record(
        &mut self,
        doc_id: t_docId,
        result: &mut RSIndexResult<'index>,
    ) -> std::io::Result<bool> {
        if !self.skip_to(doc_id) {
            return Ok(false);
        }

        let base = D::base_id(&self.ii.blocks[self.current_block_idx], self.last_doc_id);
        let success = D::seek(&mut self.current_buffer, base, doc_id, result)?;

        if success {
            self.last_doc_id = result.doc_id;
        }

        Ok(success)
    }

    fn skip_to(&mut self, doc_id: t_docId) -> bool {
        if self.ii.blocks.is_empty() {
            return false;
        }

        if self.ii.blocks[self.current_block_idx].last_doc_id >= doc_id {
            // We are already in the correct block
            return true;
        }

        // SAFETY: it is safe to unwrap because we checked that the blocks are not empty when
        // creating the reader.
        if self.ii.blocks.last().unwrap().last_doc_id < doc_id {
            // The document ID is greater than the last document ID in the index
            return false;
        }

        // Check if the very next block is correct before doing a binary search. This is a small
        // optimization for the common case where we are skipping to the next block.
        let search_start = self.current_block_idx + 1;
        if let Some(next_block) = self.ii.blocks.get(search_start)
            && next_block.last_doc_id >= doc_id
        {
            self.set_current_block(search_start);
            return true;
        }

        // Binary search to find the correct block index
        let relative_idx = self.ii.blocks[search_start..]
            .binary_search_by_key(&doc_id, |b| b.last_doc_id)
            .unwrap_or_else(|insertion_point| insertion_point);

        self.set_current_block(search_start + relative_idx);

        true
    }

    fn reset(&mut self) {
        if !self.ii.blocks.is_empty() {
            self.set_current_block(0);
        } else {
            self.current_buffer = Cursor::new(&[]);
            self.last_doc_id = 0;
        }

        self.gc_marker = self.ii.gc_marker.load(atomic::Ordering::Relaxed);
    }

    fn unique_docs(&self) -> u64 {
        self.ii.unique_docs() as u64
    }

    fn has_duplicates(&self) -> bool {
        self.ii.flags() & IndexFlags_Index_HasMultiValue > 0
    }

    fn flags(&self) -> IndexFlags {
        self.ii.flags()
    }

    fn needs_revalidation(&self) -> bool {
        self.gc_marker != self.ii.gc_marker.load(atomic::Ordering::Relaxed)
    }

    fn refresh_buffer_pointers(&mut self) {
        if !self.ii.blocks.is_empty() && self.current_block_idx < self.ii.blocks.len() {
            let current_block = &self.ii.blocks[self.current_block_idx];
            // Update the cursor to point to the current position in the refreshed buffer
            let position = self.current_buffer.position();
            self.current_buffer = Cursor::new(&current_block.buffer);
            self.current_buffer.set_position(position);
        }
    }
}

impl<'index, E: DecodedBy<Decoder = D>, D: Decoder> IndexReaderCore<'index, E> {
    /// Create a new index reader that reads from the given [`InvertedIndex`].
    ///
    /// # Panic
    /// This function will panic if the inverted index is empty.
    pub(crate) fn new(ii: &'index InvertedIndex<E>) -> Self {
        let (current_buffer, last_doc_id) = if let Some(first_block) = ii.blocks.first() {
            (
                Cursor::new(first_block.buffer.as_ref()),
                first_block.first_doc_id,
            )
        } else {
            (Cursor::new(&[] as &[u8]), 0)
        };

        Self {
            ii,
            current_buffer,
            current_block_idx: 0,
            last_doc_id,
            gc_marker: ii.gc_marker.load(atomic::Ordering::Relaxed),
        }
    }

    /// Check if this reader is reading from the given index
    pub fn is_index(&self, index: &InvertedIndex<E>) -> bool {
        std::ptr::eq(self.ii, index)
    }

    /// Swap the inverted index of the reader with the supplied index. This is only used by the C
    /// tests to trigger a revalidation.
    pub const fn swap_index(&mut self, index: &mut &'index InvertedIndex<E>) {
        std::mem::swap(&mut self.ii, index);
    }

    /// Get the internal index of the reader. This is only used by some C tests.
    pub const fn internal_index(&self) -> &InvertedIndex<E> {
        self.ii
    }

    /// Set the current active block to the given index
    fn set_current_block(&mut self, index: usize) {
        debug_assert!(
            index < self.ii.blocks.len(),
            "block index should stay in bounds"
        );

        self.current_block_idx = index;
        let current_block = &self.ii.blocks[self.current_block_idx];
        self.last_doc_id = current_block.first_doc_id;
        self.current_buffer = Cursor::new(&current_block.buffer);
    }
}
