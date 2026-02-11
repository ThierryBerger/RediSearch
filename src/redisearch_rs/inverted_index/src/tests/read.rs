/*
 * Copyright (c) 2006-Present, Redis Ltd.
 * All rights reserved.
 *
 * Licensed under your choice of the Redis Source Available License 2.0
 * (RSALv2); or (b) the Server Side Public License v1 (SSPLv1); or (c) the
 * GNU Affero General Public License v3 (AGPLv3).
*/

use std::io::{Cursor, Read};

use crate::{Decoder, Encoder, IndexBlock, IndexReader, InvertedIndex, RSIndexResult};
use ffi::IndexFlags_Index_DocIdsOnly;
use pretty_assertions::assert_eq;
use thin_vec::medium_thin_vec;

use super::Dummy;

#[test]
fn reading_records() {
    // Make two blocks. The first with two records and the second with one record
    let blocks = medium_thin_vec![
        IndexBlock {
            buffer: vec![0, 0, 0, 0, 0, 0, 0, 1],
            num_entries: 2,
            first_doc_id: 10,
            last_doc_id: 11,
        },
        IndexBlock {
            buffer: vec![0, 0, 0, 0],
            num_entries: 0,
            first_doc_id: 100,
            last_doc_id: 100,
        },
    ];
    let ii = InvertedIndex::<Dummy>::from_blocks(IndexFlags_Index_DocIdsOnly, blocks);
    let mut ir = ii.reader();
    let mut result = RSIndexResult::default();

    let found = ir
        .next_record(&mut result)
        .expect("to be able to read from the buffer");
    assert!(found);
    assert_eq!(result, RSIndexResult::virt().doc_id(10));

    let found = ir
        .next_record(&mut result)
        .expect("to be able to read from the buffer");
    assert!(found);
    assert_eq!(result, RSIndexResult::virt().doc_id(11));

    let found = ir
        .next_record(&mut result)
        .expect("to be able to read from the buffer");
    assert!(found);
    assert_eq!(result, RSIndexResult::virt().doc_id(100));

    let found = ir
        .next_record(&mut result)
        .expect("to be able to read from the buffer");
    assert!(!found);
}

#[test]
fn reading_over_empty_blocks() {
    // Make three blocks with the second one being empty and the other two containing one entries.
    // The second should automatically continue from the third block
    let blocks = medium_thin_vec![
        IndexBlock {
            buffer: vec![0, 0, 0, 0],
            num_entries: 1,
            first_doc_id: 10,
            last_doc_id: 10,
        },
        IndexBlock {
            buffer: vec![0, 0, 0, 0],
            num_entries: 1,
            first_doc_id: 30,
            last_doc_id: 30,
        },
    ];
    let ii = InvertedIndex::<Dummy>::from_blocks(IndexFlags_Index_DocIdsOnly, blocks);
    let mut ir = ii.reader();
    let mut result = RSIndexResult::default();

    let found = ir
        .next_record(&mut result)
        .expect("to be able to read from the buffer");
    assert!(found);
    assert_eq!(result, RSIndexResult::virt().doc_id(10));

    let found = ir
        .next_record(&mut result)
        .expect("to be able to read from the buffer");
    assert!(found);
    assert_eq!(result, RSIndexResult::virt().doc_id(30));

    let found = ir
        .next_record(&mut result)
        .expect("to be able to read from the buffer");
    assert!(!found, "should not return any more records");
}

#[test]
fn read_using_the_first_block_id_as_the_base() {
    #[derive(Clone)]
    struct FirstBlockIdDummy;

    impl Encoder for FirstBlockIdDummy {
        type Delta = u32;

        fn encode<W: std::io::Write + std::io::Seek>(
            _writer: W,
            _delta: Self::Delta,
            _record: &RSIndexResult,
        ) -> std::io::Result<usize> {
            panic!("This test won't encode anything")
        }
    }

    impl Decoder for FirstBlockIdDummy {
        fn decode<'index>(
            cursor: &mut Cursor<&'index [u8]>,
            prev_doc_id: u64,
            result: &mut RSIndexResult<'index>,
        ) -> std::io::Result<()> {
            let mut buffer = [0; 4];
            cursor.read_exact(&mut buffer)?;

            let delta = u32::from_be_bytes(buffer);
            let doc_id = prev_doc_id + (delta as u64);

            result.doc_id = doc_id;

            Ok(())
        }

        fn base_result<'index>() -> RSIndexResult<'index> {
            RSIndexResult::default()
        }

        fn base_id(block: &IndexBlock, _last_doc_id: ffi::t_docId) -> ffi::t_docId {
            block.first_doc_id
        }
    }

    // Make a block with three different doc IDs
    let blocks = medium_thin_vec![IndexBlock {
        buffer: vec![0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 2],
        num_entries: 3,
        first_doc_id: 10,
        last_doc_id: 12,
    }];
    let ii = InvertedIndex::<FirstBlockIdDummy>::from_blocks(IndexFlags_Index_DocIdsOnly, blocks);
    let mut ir = ii.reader();
    let mut result = RSIndexResult::default();

    let found = ir
        .next_record(&mut result)
        .expect("to be able to read from the buffer");
    assert!(found);
    assert_eq!(result, RSIndexResult::virt().doc_id(10));

    let found = ir
        .next_record(&mut result)
        .expect("to be able to read from the buffer");
    assert!(found);
    assert_eq!(result, RSIndexResult::virt().doc_id(11));

    let found = ir
        .next_record(&mut result)
        .expect("to be able to read from the buffer");
    assert!(found);
    assert_eq!(result, RSIndexResult::virt().doc_id(12));
}
