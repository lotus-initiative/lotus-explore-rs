// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `lotus curate`, in their own file.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing
)]

use super::{
    CuratedReport, Finding, convertible_smiles, should_warn_about_unchecked, unchecked_count,
};
use lotus_curation::{CurationInputRow, CurationResultRow, CurationStatus};

fn finding(smiles: &str) -> Finding {
    Finding {
        name: "Quercetin".into(),
        smiles: smiles.into(),
        taxon: None,
        doi: None,
    }
}

fn row(status: &CurationStatus) -> CurationResultRow {
    CurationResultRow {
        input: CurationInputRow {
            name: "Quercetin".into(),
            smiles: "CCO".into(),
            taxon: None,
            doi: None,
        },
        canonical_smiles: None,
        inchikey: None,
        inchi: None,
        formula: None,
        exact_mass: None,
        mass_warning: None,
        wikidata_qid: None,
        status: status.clone(),
        note: String::new(),
        dependency_blocks: vec![],
        quickstatements: vec![],
    }
}

fn report_with(statuses: &[CurationStatus]) -> CuratedReport {
    CuratedReport {
        rows: statuses.iter().map(row).collect(),
        statements: vec![],
        citation: "citation",
    }
}

#[test]
fn only_the_rows_that_were_not_checked_are_counted() {
    // The statuses a row can end in. Counting the wrong one makes the
    // warning a lie in either direction: silent about rows that were never
    // looked up, or claiming failures that are not.
    let report = report_with(&[
        CurationStatus::ExistingComplete,
        CurationStatus::NotChecked,
        CurationStatus::NewCompound,
        CurationStatus::NotChecked,
    ]);
    assert_eq!(unchecked_count(&report), 2);
}

#[test]
fn a_fully_checked_report_counts_zero() {
    // Which is the case the `> 0` decides: nothing to say, so nothing is
    // printed. A `>= 0` here would announce "0 row(s) were not looked up" on
    // every successful run.
    let report = report_with(&[
        CurationStatus::ExistingComplete,
        CurationStatus::ExistingNeedsUpdates,
        CurationStatus::NewCompound,
        CurationStatus::PendingDependencies,
    ]);
    assert_eq!(unchecked_count(&report), 0);
}

#[test]
fn the_warning_needs_something_to_say_and_permission_to_say_it() {
    // A clean run says nothing, however loud. A quiet run says nothing,
    // however many it skipped. Anything else says so.
    assert!(
        !should_warn_about_unchecked(0, false),
        "nothing was skipped"
    );
    assert!(
        !should_warn_about_unchecked(0, true),
        "nothing was skipped, and quiet"
    );
    assert!(!should_warn_about_unchecked(3, true), "quiet means quiet");
    assert!(
        should_warn_about_unchecked(1, false),
        "one skipped is still one"
    );
    assert!(should_warn_about_unchecked(3, false));
}

#[test]
fn an_empty_report_counts_zero() {
    let empty = CuratedReport {
        rows: vec![],
        statements: vec![],
        citation: "citation",
    };
    assert_eq!(unchecked_count(&empty), 0);
}

#[test]
fn blank_structures_are_not_sent_to_be_converted() {
    // Each of these would be a request the service cannot satisfy, answered
    // with the error that the structure was empty.
    let findings = vec![
        finding("CCO"),
        finding(""),
        finding("   "),
        finding("\t\n"),
        finding("CCC"),
    ];
    assert_eq!(convertible_smiles(&findings), ["CCO", "CCC"]);
}

#[test]
fn a_surrounding_space_is_trimmed_rather_than_sent() {
    // Padded SMILES from a spreadsheet, which is where most of them come
    // from. The service is asked about the structure, not the padding.
    assert_eq!(convertible_smiles(&[finding("  CCO  ")]), ["CCO"]);
    assert_eq!(convertible_smiles(&[finding("\tCCO\n")]), ["CCO"]);
}

#[test]
fn a_batch_with_nothing_convertible_sends_nothing() {
    // A batch of blanks from a spreadsheet is exactly the case where an
    // empty request is the right answer rather than a request that fails.
    assert_eq!(
        convertible_smiles(&[finding(""), finding(" ")]),
        Vec::<&str>::new()
    );
    assert_eq!(convertible_smiles(&[]).len(), 0, "expected no entries");
}
