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
    CurateArgs, CuratedReport, Finding, convertible_smiles, should_warn_about_unchecked,
    unchecked_count,
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

/// The six status keys, all six arms.
///
/// Four of these had no test. They are a wire format: the doc says the strings
/// are what "a person greps for and what a downstream script switches on", which
/// makes a renamed arm a silent break for anything reading the output, and no
/// test could tell.
///
/// Duplicated knowledge, recorded rather than acted on: the web client carries a
/// second copy of these six strings in
/// `apps/lotus-explore-rs/src/components/curation_results_table.rs`. The two
/// crates cannot see each other -- the app does not depend on `lotus-cli` -- so
/// nothing can assert they agree, and a test in either crate cannot catch a
/// divergence. Worth a decision by a human: one shared table, or an accepted
/// duplication with the strings written down somewhere both can read.
#[test]
fn every_status_has_a_stable_key() {
    use lotus_curation::CurationStatus;
    for (status, expected) in [
        (CurationStatus::ExistingComplete, "existing_complete"),
        (CurationStatus::ExistingNeedsUpdates, "existing_updates"),
        (CurationStatus::NewCompound, "new_compound"),
        (CurationStatus::PendingDependencies, "pending_dependencies"),
        (CurationStatus::NotChecked, "not_checked"),
        (CurationStatus::Error, "error"),
    ] {
        assert_eq!(
            super::status_key(&status),
            expected,
            "{status:?} has the wrong key: a downstream script switching on it breaks"
        );
    }
}

/// The keys are also distinct, so a script can tell two statuses apart.
///
/// Cheap to assert and the thing a rename would actually collide with: two arms
/// agreeing would make `existing_updates` and `existing_complete`
/// indistinguishable in a report, and both are common.
#[test]
fn no_two_statuses_share_a_key() {
    use lotus_curation::CurationStatus;
    let all = [
        CurationStatus::ExistingComplete,
        CurationStatus::ExistingNeedsUpdates,
        CurationStatus::NewCompound,
        CurationStatus::PendingDependencies,
        CurationStatus::NotChecked,
        CurationStatus::Error,
    ];
    for (i, a) in all.iter().enumerate() {
        for b in all.iter().skip(i + 1) {
            assert_ne!(
                super::status_key(a),
                super::status_key(b),
                "{a:?} and {b:?} render identically, so a report cannot distinguish them"
            );
        }
    }
}

/// An input with no usable rows is a failure, not an empty report.
///
/// `parse_tsv` accepts a header it understands, so a file of nothing but the
/// header parses to zero findings. Reporting success with no output would look
/// like "checked your rows, there was nothing to say", which is a different and
/// wrong claim from "I found no rows in what you gave me".
#[tokio::test]
async fn an_input_with_no_usable_rows_fails_rather_than_reporting_nothing() {
    let dir = temp_dir("no-usable-rows");
    let input = dir.join("empty.tsv");
    std::fs::write(&input, "name\tsmiles\ttaxon\tdoi\n").expect("the fixture is writable");

    let args = CurateArgs {
        input: Some(input.display().to_string()),
        offline: true,
        quiet: true,
        format: super::ReportFormat::Table,
        output: None,
        log: super::LogLevel::Warn,
    };

    let code = super::run(&args).await.expect("run does not error");
    assert_eq!(
        format!("{code:?}"),
        format!("{:?}", std::process::ExitCode::FAILURE),
        "zero usable rows must be a failure exit"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// A path that does not exist is an error naming the path.
///
/// The user needs to know *which* file: `lotus curate typo.tsv` is a typo, and
/// "No such file or directory" alone does not say so.
#[tokio::test]
async fn a_missing_input_file_names_the_path_it_could_not_read() {
    let args = CurateArgs {
        input: Some("lotus-nonexistent-input.tsv".to_string()),
        offline: true,
        quiet: true,
        format: super::ReportFormat::Table,
        output: None,
        log: super::LogLevel::Warn,
    };

    let error = super::run(&args).await.expect_err("a missing file errors");
    let rendered = error.to_string();
    assert!(
        rendered.contains("lotus-nonexistent-input.tsv"),
        "the error must name the file: {rendered:?}"
    );
}

/// A scratch directory under the temp dir, removed by the caller.
///
/// Hand-rolled rather than a `tempfile` dev-dependency: the earlier session
/// removed `tempfile` from this crate for having no call site, and three call
/// sites now is not a reason to put a dependency and a transitive graph back.
fn temp_dir(label: &str) -> std::path::PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!("lotus-curate-{label}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("the temp directory is creatable");
    dir
}
