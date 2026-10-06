// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `search_utils`, in their own file.

use super::*;
use std::sync::Arc;

#[test]
fn sanitizes_taxon_prefix_case_only() {
    assert_eq!(
        sanitize_taxon_input("voacanga africana"),
        "Voacanga africana"
    );
    assert_eq!(
        sanitize_taxon_input("VOACANGA africana"),
        "Voacanga africana"
    );
    assert_eq!(sanitize_taxon_input("__gentiana_lutea__"), "Gentiana lutea");
}

#[test]
fn hex_encoder_is_lowercase() {
    assert_eq!(to_hex_lower(&[0xAB, 0xCD, 0xEF]), "abcdef");
}

#[test]
fn hashes_depend_on_query_and_rows_only() {
    let crit = SearchCriteria {
        taxon: "*".into(),
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };
    let row = CompoundEntry {
        compound_qid: Arc::from("Q1"),
        name: Arc::from("A"),
        inchikey: None,
        smiles: None,
        mass: None,
        formula: None,
        taxon_qid: Arc::from("Q2"),
        taxon_name: Arc::from("Taxon"),
        reference_qid: Arc::from("Q3"),
        reference_node: Arc::from(""),
        ref_title: None,
        ref_doi: None,
        pub_year: None,
        statement: None,
    };
    let (q1, r1) = compute_hashes(
        "QX",
        &crit,
        &ColumnarResultSet::from_entries(std::slice::from_ref(&row)),
    );
    let (q2, r2) = compute_hashes("QX", &crit, &ColumnarResultSet::from_entries(&[row]));
    assert_eq!(q1, q2);
    assert_eq!(r1, r2);
}
