/*
 * Copyright (c) 2006-Present, Redis Ltd.
 * All rights reserved.
 *
 * Licensed under your choice of the Redis Source Available License 2.0
 * (RSALv2); or (b) the Server Side Public License v1 (SSPLv1); or (c) the
 * GNU Affero General Public License v3 (AGPLv3).
*/

use crate::{
    Encoder, EntriesTrackingIndex, InvertedIndex, RSAggregateResult, RSIndexResult, RSResultData,
    RSResultKind, RSTermRecord,
    debug::{BlockSummary, Summary},
};
use ffi::{IndexFlags_Index_DocIdsOnly, IndexFlags_Index_StoreNumeric};
use pretty_assertions::assert_eq;

use super::Dummy;

#[test]
fn synced_discriminants() {
    let tests = [
        (
            RSResultData::Union(RSAggregateResult::with_capacity(0)),
            RSResultKind::Union,
        ),
        (
            RSResultData::Intersection(RSAggregateResult::with_capacity(0)),
            RSResultKind::Intersection,
        ),
        (
            RSResultData::Term(RSTermRecord::default()),
            RSResultKind::Term,
        ),
        (RSResultData::Virtual, RSResultKind::Virtual),
        (RSResultData::Numeric(0f64), RSResultKind::Numeric),
        (RSResultData::Metric(0f64), RSResultKind::Metric),
        (
            RSResultData::HybridMetric(RSAggregateResult::with_capacity(0)),
            RSResultKind::HybridMetric,
        ),
    ];

    for (data, kind) in tests {
        assert_eq!(data.kind(), kind);

        // SAFETY: since `RSResultData` is a `repr(u8)` it will have a C `union` layout with a
        // `repr(C)` struct for each variant. Each of these structs has the discriminant as the
        // first field, which we can read here without offsetting the pointer.
        //
        // For more see https://doc.rust-lang.org/std/mem/fn.discriminant.html#accessing-the-numeric-value-of-the-discriminant
        let data_discriminant = unsafe { *<*const _>::from(&data).cast::<u8>() };
        let kind_discriminant = kind as u8;

        assert_eq!(data_discriminant, kind_discriminant, "for {kind:?}");
    }
}

#[test]
fn summary() {
    let mut ii = InvertedIndex::<Dummy>::new(IndexFlags_Index_DocIdsOnly);

    assert_eq!(
        ii.summary(),
        Summary {
            number_of_docs: 0,
            number_of_entries: 0,
            last_doc_id: 0,
            flags: IndexFlags_Index_DocIdsOnly as _,
            number_of_blocks: 0,
            block_efficiency: 0.0,
            has_efficiency: false,
        }
    );

    let record = RSIndexResult::default().doc_id(10);
    let _mem_growth = ii.add_record(&record).unwrap();

    let record = RSIndexResult::default().doc_id(11);
    let _mem_growth = ii.add_record(&record).unwrap();

    assert_eq!(
        ii.summary(),
        Summary {
            number_of_docs: 2,
            number_of_entries: 2,
            last_doc_id: 11,
            flags: IndexFlags_Index_DocIdsOnly as _,
            number_of_blocks: 1,
            block_efficiency: 0.0,
            has_efficiency: false,
        }
    );
}

#[test]
fn summary_store_numeric() {
    let mut ii = EntriesTrackingIndex::<Dummy>::new(IndexFlags_Index_StoreNumeric);

    assert_eq!(
        ii.summary(),
        Summary {
            number_of_docs: 0,
            number_of_entries: 0,
            last_doc_id: 0,
            flags: IndexFlags_Index_StoreNumeric as _,
            number_of_blocks: 0,
            block_efficiency: 0.0,
            has_efficiency: true,
        }
    );

    let record = RSIndexResult::default().doc_id(10);
    let _mem_growth = ii.add_record(&record).unwrap();

    let record = RSIndexResult::default().doc_id(10);
    let _mem_growth = ii.add_record(&record).unwrap();

    assert_eq!(
        ii.summary(),
        Summary {
            number_of_docs: 1,
            number_of_entries: 2,
            last_doc_id: 10,
            flags: IndexFlags_Index_StoreNumeric as _,
            number_of_blocks: 1,
            block_efficiency: 2.0,
            has_efficiency: true,
        }
    );
}

#[test]
fn blocks_summary() {
    /// Dummy encoder which only allows 2 entries per block for testing
    #[derive(Clone)]
    struct SmallBlocksDummy;

    impl Encoder for SmallBlocksDummy {
        type Delta = u32;

        const ALLOW_DUPLICATES: bool = true;
        const RECOMMENDED_BLOCK_ENTRIES: u16 = 2;

        fn encode<W: std::io::Write + std::io::Seek>(
            mut writer: W,
            _delta: Self::Delta,
            _record: &RSIndexResult,
        ) -> std::io::Result<usize> {
            writer.write_all(&[1])?;

            Ok(1)
        }
    }

    let mut ii = InvertedIndex::<SmallBlocksDummy>::new(IndexFlags_Index_DocIdsOnly);

    assert_eq!(ii.blocks_summary().len(), 0);

    let record = RSIndexResult::default().doc_id(10);
    let _mem_growth = ii.add_record(&record).unwrap();

    let record = RSIndexResult::default().doc_id(11);
    let _mem_growth = ii.add_record(&record).unwrap();

    let record = RSIndexResult::default().doc_id(12);
    let _mem_growth = ii.add_record(&record).unwrap();

    let summaries = ii.blocks_summary();
    assert_eq!(
        summaries,
        vec![
            BlockSummary {
                first_doc_id: 10,
                last_doc_id: 11,
                number_of_entries: 2,
            },
            BlockSummary {
                first_doc_id: 12,
                last_doc_id: 12,
                number_of_entries: 1,
            }
        ]
    );
}

#[test]
fn blocks_summary_store_numeric() {
    /// Dummy encoder which only allows 2 entries per block for testing
    #[derive(Clone)]
    struct SmallBlocksDummy;

    impl Encoder for SmallBlocksDummy {
        type Delta = u32;

        const ALLOW_DUPLICATES: bool = true;
        const RECOMMENDED_BLOCK_ENTRIES: u16 = 2;

        fn encode<W: std::io::Write + std::io::Seek>(
            mut writer: W,
            _delta: Self::Delta,
            _record: &RSIndexResult,
        ) -> std::io::Result<usize> {
            writer.write_all(&[1])?;

            Ok(1)
        }
    }

    let mut ii = EntriesTrackingIndex::<SmallBlocksDummy>::new(IndexFlags_Index_StoreNumeric);

    assert_eq!(ii.blocks_summary().len(), 0);

    let record = RSIndexResult::default().doc_id(10);
    let _mem_growth = ii.add_record(&record).unwrap();

    let record = RSIndexResult::default().doc_id(11);
    let _mem_growth = ii.add_record(&record).unwrap();

    let record = RSIndexResult::default().doc_id(12);
    let _mem_growth = ii.add_record(&record).unwrap();

    let summaries = ii.blocks_summary();
    assert_eq!(
        summaries,
        vec![
            BlockSummary {
                first_doc_id: 10,
                last_doc_id: 11,
                number_of_entries: 2,
            },
            BlockSummary {
                first_doc_id: 12,
                last_doc_id: 12,
                number_of_entries: 1,
            }
        ]
    );
}
