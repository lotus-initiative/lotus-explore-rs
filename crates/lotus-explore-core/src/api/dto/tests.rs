// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `dto`, in their own file.

#![allow(clippy::expect_used)]

use super::*;

/// The year the tests pin, so "is this year bound in the future?" has one
/// answer rather than the answer the day the suite runs.
const YEAR: u16 = 2026;

#[test]
fn request_builder_keeps_large_formula_ranges() {
    let mut criteria = SearchCriteria {
        taxon: "*".into(),
        ..SearchCriteria::up_to_year(YEAR)
    };
    criteria.formula_enabled = true;
    criteria.c_max = 300;
    criteria.h_max = 900;

    let request = SearchRequest::from_criteria(&criteria, 123, true, YEAR);
    assert_eq!(request.c_max, Some(300));
    assert_eq!(request.h_max, Some(900));
    assert_eq!(request.limit, Some(123));
    assert_eq!(request.include_counts, Some(true));
}

#[test]
fn request_builder_preserves_multiline_molfile_whitespace() {
    let criteria = SearchCriteria {
        taxon: "*".into(),
        structure: "\n  Mrv\n\n  0  0  0  0  0  0            999 V3000\nM  END\n".into(),
        ..SearchCriteria::up_to_year(YEAR)
    };

    let request = SearchRequest::from_criteria(&criteria, 10, false, YEAR);
    let smiles = request.structure.expect("smiles payload");
    assert!(smiles.starts_with('\n'));
    assert!(smiles.contains("V3000"));
}

#[test]
fn request_builder_omits_the_year_bounds_when_no_year_filter_is_set() {
    let criteria = SearchCriteria {
        taxon: "*".into(),
        ..SearchCriteria::up_to_year(YEAR)
    };

    let request = SearchRequest::from_criteria(&criteria, 10, false, YEAR);
    assert_eq!(request.year_min, None);
    assert_eq!(request.year_max, None);
}

#[test]
fn normalize_statement_strips_wikidata_prefix() {
    assert_eq!(
        normalize_statement(Some(
            "http://www.wikidata.org/entity/statement/S123".to_string()
        )),
        Some("S123".to_string())
    );
    assert_eq!(
        normalize_statement(Some("S124".to_string())),
        Some("S124".to_string())
    );
    assert_eq!(normalize_statement(Some("   ".to_string())), None);
}
