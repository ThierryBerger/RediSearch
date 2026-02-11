/*
 * Copyright (c) 2006-Present, Redis Ltd.
 * All rights reserved.
 *
 * Licensed under your choice of the Redis Source Available License 2.0
 * (RSALv2); or (b) the Server Side Public License v1 (SSPLv1); or (c) the
 * GNU Affero General Public License v3 (AGPLv3).
*/

use std::ptr;

use crate::{
    FilterGeoReader, FilterMaskReader, FilterNumericReader, IndexReader, NumericFilter,
    RSIndexResult,
};
use ffi::{GeoDistance_GEO_DISTANCE_M, GeoFilter};
use pretty_assertions::assert_eq;

#[test]
fn reading_filter_based_on_field_mask() {
    // Make an iterator with three records having different field masks. The second record will be
    // filtered out based on the field mask.
    let iter = vec![
        RSIndexResult::default().doc_id(10).field_mask(0b0001),
        RSIndexResult::default().doc_id(11).field_mask(0b0010),
        RSIndexResult::default().doc_id(12).field_mask(0b0100),
    ];

    let mut reader = FilterMaskReader::new(0b0101 as _, iter.into_iter());
    let mut result = RSIndexResult::default();

    let found = reader.next_record(&mut result).unwrap();
    assert!(found);
    assert_eq!(
        result,
        RSIndexResult::default().doc_id(10).field_mask(0b0001)
    );

    let found = reader.next_record(&mut result).unwrap();
    assert!(found);
    assert_eq!(
        result,
        RSIndexResult::default().doc_id(12).field_mask(0b0100)
    );
}

#[test]
fn reading_filter_based_on_numeric_filter() {
    // Make an iterator with three records having different numeric values. The second record will be
    // filtered out based on the numeric filter.
    let iter = vec![
        RSIndexResult::numeric(5.0).doc_id(10),
        RSIndexResult::numeric(25.0).doc_id(11),
        RSIndexResult::numeric(15.0).doc_id(12),
    ];

    let filter = NumericFilter {
        min: 0.0,
        max: 15.0,
        min_inclusive: true,
        max_inclusive: true,
        field_spec: ptr::null(),
        geo_filter: ptr::null(),
        ascending: true,
        limit: 10,
        offset: 0,
    };

    let mut reader = FilterNumericReader::new(&filter, iter.into_iter());
    let mut result = RSIndexResult::numeric(0.0);

    let found = reader.next_record(&mut result).unwrap();
    assert!(found);
    assert_eq!(result, RSIndexResult::numeric(5.0).doc_id(10));

    let found = reader.next_record(&mut result).unwrap();
    assert!(found);
    assert_eq!(result, RSIndexResult::numeric(15.0).doc_id(12));
}

#[test]
fn reading_filter_based_on_geo_filter() {
    /// Implement this FFI call for this test
    #[unsafe(no_mangle)]
    pub extern "C" fn isWithinRadius(gf: *const GeoFilter, d: f64, distance: *mut f64) -> bool {
        if d > unsafe { (*gf).radius } {
            return false;
        }

        // Tests changing the distance value
        unsafe { *distance /= 5.0 };

        true
    }

    // Make an iterator with three records having different geo distances. The last record will be
    // filtered out based on the geo distance.
    let iter = vec![
        RSIndexResult::numeric(5.0).doc_id(10),
        RSIndexResult::numeric(15.0).doc_id(11),
        RSIndexResult::numeric(25.0).doc_id(12),
    ];

    let geo_filter = GeoFilter {
        fieldSpec: ptr::null(),
        lat: 0.0,
        lon: 0.0,
        radius: 20.0,
        unitType: GeoDistance_GEO_DISTANCE_M,
        numericFilters: ptr::null_mut(),
    };

    let filter = NumericFilter {
        min: 0.0,
        max: 0.0,
        min_inclusive: false,
        max_inclusive: false,
        field_spec: ptr::null(),
        geo_filter: &geo_filter as *const _ as *const _,
        ascending: true,
        limit: 0,
        offset: 0,
    };

    let mut reader = FilterGeoReader::new(&filter, iter.into_iter());
    let mut result = RSIndexResult::numeric(0.0);

    let found = reader.next_record(&mut result).unwrap();
    assert!(found);
    assert_eq!(result, RSIndexResult::numeric(1.0).doc_id(10));

    let found = reader.next_record(&mut result).unwrap();
    assert!(found);
    assert_eq!(result, RSIndexResult::numeric(3.0).doc_id(11));
}
