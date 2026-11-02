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

fn row(compound_qid: &str) -> CompoundEntry {
    CompoundEntry {
        compound_qid: Arc::from(compound_qid),
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
    }
}

#[test]
fn hashes_depend_on_query_and_rows_only() {
    let crit = SearchCriteria {
        taxon: "*".into(),
        ..SearchCriteria::up_to_year(2026)
    };
    let (q1, r1) = compute_hashes(
        "QX",
        &crit,
        &ColumnarResultSet::from_entries(&[row("Q1")]),
        2026,
    );
    let (q2, r2) = compute_hashes(
        "QX",
        &crit,
        &ColumnarResultSet::from_entries(&[row("Q1")]),
        2026,
    );
    assert_eq!(q1, q2);
    assert_eq!(r1, r2);
}

/// The hashes are the SHA-256 of `qid|taxon|k=v&k=v` and of `Q1|Q2`: the
/// expected digests are those of exactly these strings, so a changed separator
/// or a dropped one changes every shared link and must fail here.
#[test]
fn hashes_are_the_digests_of_the_documented_source_strings() {
    let crit = SearchCriteria {
        taxon: "Gentiana".into(),
        reference: "Q1".into(),
        ..SearchCriteria::up_to_year(2026)
    };
    let set = ColumnarResultSet::from_entries(&[row("Q1"), row("Q2")]);

    // sha256("QX|Gentiana|reference=Q1&taxon=Gentiana") and sha256("Q1|Q2").
    assert_eq!(
        compute_hashes("QX", &crit, &set, 2026),
        (
            "c78e70977f1424c462c09bfa508368a28d08308ec79a57e3d8c3cbbe14f98961".to_owned(),
            "2e343f3e25287c295d4134e07256cf134656ea81d7a6b46dbfc28c5b105122e4".to_owned(),
        )
    );
    // A blank QID hashes as `*`: sha256("*|Gentiana|reference=Q1&taxon=Gentiana").
    assert_eq!(
        compute_hashes("  ", &crit, &set, 2026).0,
        "84bee56745783daf96339348ca1ecab6b4c857a3c9baff6fb7580462025750fa"
    );
}
