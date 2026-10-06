// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `occurrence`, in their own file.

use super::*;

#[test]
fn a_taxon_and_a_reference_both_land_in_the_statement() {
    let resolved = Resolved {
        taxon_qid: Some("Q1"),
        reference_qid: Some("Q2"),
        row_has_doi: true,
    };
    assert_eq!(
        resolved.statement_for("Q9").as_deref(),
        Some("Q9|P703|Q1|S248|Q2"),
        "a statement with a reference is qualified by it"
    );
}

#[test]
fn a_taxon_without_a_reference_is_unqualified() {
    let resolved = Resolved {
        taxon_qid: Some("Q1"),
        reference_qid: None,
        row_has_doi: false,
    };
    assert_eq!(resolved.statement_for("Q9").as_deref(), Some("Q9|P703|Q1"));
}

#[test]
fn a_doi_with_no_item_defers_the_statement() {
    let resolved = Resolved {
        taxon_qid: Some("Q1"),
        reference_qid: None,
        row_has_doi: true,
    };
    assert_eq!(
        resolved.statement_for("Q9"),
        None,
        "P703 cannot name a reference that does not exist yet"
    );
}

#[test]
fn a_missing_taxon_defers_the_statement() {
    let resolved = Resolved {
        taxon_qid: None,
        reference_qid: Some("Q2"),
        row_has_doi: true,
    };
    assert_eq!(
        resolved.statement_for("Q9").as_deref(),
        Some("Q9|P703|[NEW_TAXON_QID]"),
        "the placeholder is filled in on the next pass"
    );
}

#[test]
fn a_new_compound_writes_against_last() {
    let resolved = Resolved {
        taxon_qid: Some("Q1"),
        reference_qid: None,
        row_has_doi: false,
    };
    assert_eq!(
        resolved.statement_for_new().as_deref(),
        Some("LAST|P703|Q1")
    );
}
