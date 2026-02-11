/*
 * Copyright (c) 2006-Present, Redis Ltd.
 * All rights reserved.
 *
 * Licensed under your choice of the Redis Source Available License 2.0
 * (RSALv2); or (b) the Server Side Public License v1 (SSPLv1); or (c) the
 * GNU Affero General Public License v3 (AGPLv3).
*/

pub mod controlled_cursor;
pub mod debug;
pub mod encoders;
pub mod filter;
mod gc;
mod index;
mod index_result;
pub mod opaque;
pub mod reader;
#[doc(hidden)]
pub mod test_utils;
mod wrappers;

// Re-export encoder submodules at crate root for backward compatibility.
pub use encoders::{
    doc_ids_only, fields_offsets, fields_only, freqs_fields, freqs_offsets, freqs_only, full,
    numeric, offsets_only, raw_doc_ids_only,
};

// Re-export encoder/decoder traits at crate root.
pub use encoders::codec::*;

// Re-export core index types.
pub use index::{IndexBlock, InvertedIndex};

// Re-export GC types.
pub(crate) use gc::{BlockGcScanResult, RepairType};
pub use gc::{GcApplyInfo, GcScanDelta};

// Re-export result types.
pub use index_result::{
    RSAggregateResult, RSAggregateResultIter, RSIndexResult, RSOffsetVector, RSQueryTerm,
    RSResultData, RSResultKind, RSResultKindMask, RSTermRecord, ResultMetrics_Reset_func,
};

// Re-export reader types.
pub use reader::{IndexReader, IndexReaderCore, NumericFilter, NumericReader, TermReader};

// Re-export filter types.
pub use filter::{FilterGeoReader, FilterMaskReader, FilterNumericReader, ReadFilter};

// Re-export wrapper types.
pub use wrappers::{EntriesTrackingIndex, FieldMaskTrackingIndex};

// Re-export FFI types.
pub use ffi::{t_docId, t_fieldMask};

#[cfg(test)]
mod tests;
