// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `dto`, in their own file.

#![allow(clippy::expect_used)]

use super::*;

#[test]
fn request_builder_keeps_large_formula_ranges() {
    let mut criteria = SearchCriteria {
        taxon: "*".into(),
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };
    criteria.formula_enabled = true;
    criteria.c_max = 300;
    criteria.h_max = 900;

    let request = SearchRequest::from_criteria(&criteria, 123, true);
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
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };

    let request = SearchRequest::from_criteria(&criteria, 10, false);
    let smiles = request.structure.expect("smiles payload");
    assert!(smiles.starts_with('\n'));
    assert!(smiles.contains("V3000"));
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
