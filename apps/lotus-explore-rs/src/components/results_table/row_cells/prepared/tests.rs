// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `prepared`, in their own file.

use super::*;

#[test]
fn display_name_keeps_full_trimmed_name() {
    let entry = CompoundEntry {
        compound_qid: Arc::<str>::from("Q1"),
        name: Arc::<str>::from(
            "  This compound name is intentionally much longer than sixty characters to verify it stays intact.  ",
        ),
        inchikey: None,
        smiles: None,
        mass: None,
        formula: None,
        taxon_qid: Arc::<str>::from("T1"),
        taxon_name: Arc::<str>::from("Taxon"),
        reference_qid: Arc::<str>::from("R1"),
        reference_node: Arc::from(""),
        ref_title: None,
        ref_doi: None,
        pub_year: None,
        statement: None,
    };

    let prepared = PreparedRow::from_entry(&entry);
    assert_eq!(
        prepared.display_name.as_ref(),
        "This compound name is intentionally much longer than sixty characters to verify it stays intact.",
    );
}

#[test]
fn prepared_row_caches_trimmed_and_derived_fields() {
    let entry = CompoundEntry {
        compound_qid: Arc::<str>::from("Q1"),
        name: Arc::<str>::from("  Alpha  "),
        inchikey: Some(Arc::<str>::from("AAAA-BBBB-CCCC")),
        smiles: Some(Arc::<str>::from("CCO")),
        mass: None,
        formula: None,
        taxon_qid: Arc::<str>::from("T1"),
        taxon_name: Arc::<str>::from("Taxon"),
        reference_qid: Arc::<str>::from("R1"),
        reference_node: Arc::from(""),
        ref_title: Some(Arc::<str>::from("  A fairly short title  ")),
        ref_doi: Some(Arc::<str>::from(" 10.1000/test ")),
        pub_year: None,
        statement: Some(Arc::<str>::from(
            "http://www.wikidata.org/entity/statement/Q1-ABC",
        )),
    };

    let prepared = PreparedRow::from_entry(&entry);
    assert_eq!(prepared.display_name.as_ref(), "Alpha");
    assert_eq!(prepared.doi.as_deref(), Some("10.1000/test"));
    assert_eq!(prepared.statement_id.as_deref(), Some("Q1-ABC"));
    assert!(
        prepared
            .depict_url
            .as_deref()
            .is_some_and(|url| url.contains("annotate=cip"))
    );
}
