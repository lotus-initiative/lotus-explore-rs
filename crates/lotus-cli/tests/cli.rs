// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The CLI's contract with a caller: what it accepts, what it writes to stdout,
//! and what it exits with.
//!
//! Nothing here touches the network. `--explain` builds and prints the SPARQL
//! without sending it, which is what makes a filter test possible at all.

#![allow(unused_crate_dependencies)]
// The panic lints keep library code free of panics on external input. A test
// that fails on a missing file or a bad fixture is reporting, not panicking.
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use assert_cmd::Command;
use predicates::prelude::*;

/// The binary under test, as cargo builds it.
fn lotus() -> Command {
    Command::cargo_bin("lotus").expect("the binary is built by cargo test")
}

#[test]
fn help_names_the_subcommands_and_describes_the_data_source() {
    lotus()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Wikidata"))
        .stdout(predicate::str::contains("SPARQL"))
        .stdout(predicate::str::contains("search"))
        .stdout(predicate::str::contains("curate"));
}

#[test]
fn search_help_lists_every_filter_the_explorer_offers() {
    let help = lotus().arg("search").arg("--help").assert().success();
    let help = String::from_utf8(help.get_output().stdout.clone()).expect("UTF-8");

    for flag in [
        "--taxon",
        "--structure",
        "--structure-search",
        "--threshold",
        "--mass-min",
        "--mass-max",
        "--year-min",
        "--year-max",
        "--formula",
        "--carbon",
        "--hydrogen",
        "--nitrogen",
        "--oxygen",
        "--phosphorus",
        "--sulfur",
        "--fluorine",
        "--chlorine",
        "--bromine",
        "--iodine",
        "--limit",
        "--format",
    ] {
        assert!(help.contains(flag), "{flag} is missing from search --help");
    }
}

#[test]
fn every_output_format_is_offered() {
    let help = lotus().arg("search").arg("--help").assert().success();
    let help = String::from_utf8(help.get_output().stdout.clone()).expect("UTF-8");
    for format in ["table", "tsv", "csv", "json", "jsonl", "jsonld", "query"] {
        assert!(help.contains(format), "{format} is missing from --format");
    }
}

#[test]
fn explain_prints_a_query_and_asks_for_nothing() {
    let output = lotus()
        .args(["search", "--taxon", "Gentiana lutea", "--explain"])
        .assert()
        .success();
    let query = String::from_utf8(output.get_output().stdout.clone()).expect("UTF-8");

    assert!(query.starts_with("PREFIX"), "got: {query}");
    assert!(query.contains("SELECT DISTINCT"));
    // The occurrence statement is what makes a row: compound, found-in-taxon,
    // derived-from-reference.
    assert!(query.contains("p:P703"), "got: {query}");
    assert!(query.contains("ps:P703"));
    assert!(query.contains("prov:wasDerivedFrom"));
}

#[test]
fn explain_resolves_no_taxon_so_the_query_is_the_unfiltered_shape() {
    // `--explain` is for reading a query, so it must not need an endpoint even
    // to turn a name into a QID. The ancestry filter is therefore absent and
    // the caller can see the shape rather than a resolution.
    let output = lotus()
        .args(["search", "--taxon", "Gentiana lutea", "--explain"])
        .assert()
        .success();
    let query = String::from_utf8(output.get_output().stdout.clone()).expect("UTF-8");
    assert!(!query.contains("P171"), "no taxon was resolved offline");
}

#[test]
fn every_filter_reaches_the_query() {
    let cases: [(&[&str], &[&str]); 5] = [
        (
            &["--mass-min", "100", "--mass-max", "400"],
            &["?compound_mass >= 100"],
        ),
        (
            &["--year-min", "1990", "--year-max", "2010"],
            &["YEAR(?ref_date) >= 1990"],
        ),
        (
            &["--formula", "C17H12O7"],
            &["?_formula_norm = \"C17H12O7\""],
        ),
        (
            &["--carbon", "10..20"],
            &["?_count_c >= 10 && ?_count_c <= 20"],
        ),
        (&["--bromine", "required"], &["FILTER(?_count_br > 0)"]),
    ];

    for (args, expected) in cases {
        let mut command = lotus();
        command.args(["search", "--explain"]).args(args);
        let output = command.assert().success();
        let query = String::from_utf8(output.get_output().stdout.clone()).expect("UTF-8");
        for fragment in expected {
            assert!(query.contains(fragment), "{args:?} lost {fragment}");
        }
    }
}

#[test]
fn an_element_range_turns_the_formula_filter_on_without_asking() {
    // A user who typed `--carbon 10..20` has filtered by formula. Requiring
    // `--formula` as well would silently drop the filter they asked for.
    let output = lotus()
        .args(["search", "--carbon", "5..", "--explain"])
        .assert()
        .success();
    let query = String::from_utf8(output.get_output().stdout.clone()).expect("UTF-8");
    assert!(query.contains("?_count_c"), "got: {query}");
}

#[test]
fn a_similarity_search_carries_its_cutoff_and_a_substructure_does_not() {
    let sim = lotus()
        .args([
            "search",
            "--structure",
            "c1ccccc1",
            "--structure-search",
            "similarity",
            "--threshold",
            "0.9",
            "--explain",
        ])
        .assert()
        .success();
    let query = String::from_utf8(sim.get_output().stdout.clone()).expect("UTF-8");
    assert!(query.contains("sachem:similarCompoundSearch"));
    assert!(query.contains(r#"sachem:cutoff "0.9"^^xsd:double"#));

    let sub = lotus()
        .args(["search", "--structure", "c1ccccc1", "--explain"])
        .assert()
        .success();
    let query = String::from_utf8(sub.get_output().stdout.clone()).expect("UTF-8");
    assert!(query.contains("sachem:substructureSearch"));
    assert!(!query.contains("sachem:cutoff"));
}

#[test]
fn the_limit_is_not_baked_into_the_query_that_explain_prints() {
    // The query `explain` prints is what a user pastes into an endpoint, and a
    // download of that query should return the whole result set.
    let output = lotus()
        .args(["search", "--limit", "5", "--explain"])
        .assert()
        .success();
    let query = String::from_utf8(output.get_output().stdout.clone()).expect("UTF-8");
    assert!(!query.contains("LIMIT"), "got: {query}");
}

#[test]
fn a_similarity_cutoff_outside_the_unit_interval_is_rejected() {
    lotus()
        .args([
            "search",
            "--structure",
            "c1ccccc1",
            "--structure-search",
            "similarity",
            "--threshold",
            "1.5",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("0..=1"));
}

#[test]
fn an_inverted_mass_range_is_rejected_before_anything_is_sent() {
    lotus()
        .args([
            "search",
            "--mass-min",
            "400",
            "--mass-max",
            "100",
            "--explain",
        ])
        .assert()
        .failure();
}

#[test]
fn an_unknown_flag_is_rejected() {
    lotus()
        .args(["search", "--magic"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--magic"));
}

#[test]
fn a_completion_script_is_generated_for_each_shell() {
    for shell in ["bash", "zsh", "fish", "elvish"] {
        let output = lotus().args(["completions", shell]).assert().success();
        let script = String::from_utf8(output.get_output().stdout.clone()).expect("UTF-8");
        assert!(!script.is_empty(), "{shell} produced nothing");
        assert!(
            script.contains("lotus"),
            "{shell} does not mention the command"
        );
    }
}

#[test]
fn the_manual_page_is_generated() {
    let output = lotus().arg("man").assert().success();
    let page = String::from_utf8(output.get_output().stdout.clone()).expect("UTF-8");
    assert!(page.contains(".TH"), "not a roff document");
    assert!(page.contains("lotus"));
    assert!(page.contains("search"));
}

#[test]
fn curate_is_a_dry_run_unless_told_otherwise() {
    let tsv = "name\tsmiles\ttaxon\tdoi\nQuercetin\tCCO\tGentiana lutea\t10.1/a\n";
    let output = lotus()
        .args(["curate", "-"])
        .write_stdin(tsv)
        .assert()
        .success();
    let stderr = String::from_utf8(output.get_output().stderr.clone()).expect("UTF-8");
    assert!(stderr.contains("dry run"), "got: {stderr}");
}

#[test]
fn curate_reads_stdin_and_ignores_a_row_with_no_smiles() {
    let tsv =
        "name\tsmiles\ttaxon\tdoi\nQuercetin\tCCO\tGentiana lutea\t10.1/a\nNoSmiles\t\tX\t10.1/b\n";
    let output = lotus()
        .args(["curate", "-", "--format", "tsv"])
        .write_stdin(tsv)
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).expect("UTF-8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "name\tsmiles\ttaxon\tdoi\tstatus");
    assert_eq!(lines.len(), 2, "the incomplete row was dropped: {stdout}");
    assert!(lines[1].contains("Quercetin"));
}

#[test]
fn curate_reports_a_missing_column_rather_than_reading_the_wrong_ones() {
    let output = lotus()
        .args(["curate", "-"])
        .write_stdin("smiles\ttaxon\nCCO\tX\n")
        .assert()
        .failure();
    let stderr = String::from_utf8(output.get_output().stderr.clone()).expect("UTF-8");
    assert!(stderr.contains("name"), "got: {stderr}");
}

#[test]
fn curate_emits_jsonl_one_finding_per_line() {
    let tsv = "name\tsmiles\nA\tCCO\nB\tCCN\n";
    let output = lotus()
        .args(["curate", "-", "--format", "jsonl"])
        .write_stdin(tsv)
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).expect("UTF-8");
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 2);
    for line in lines {
        let value: serde_json::Value = serde_json::from_str(line).expect("one object per line");
        assert!(value["name"].is_string());
    }
}

#[test]
fn curate_deduplicates_rows_that_name_the_same_finding() {
    // The same structure, taxon and DOI under two names is one finding: a
    // spreadsheet artefact, not two things to submit.
    let tsv = "name\tsmiles\ttaxon\tdoi\nQuercetin\tCCO\tGentiana lutea\t10.1/a\nAlso called quercetin\tcco\tGENTIANA LUTEA\t10.1/A\n";
    let output = lotus()
        .args(["curate", "-", "--format", "jsonl"])
        .write_stdin(tsv)
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).expect("UTF-8");
    assert_eq!(stdout.lines().filter(|l| !l.is_empty()).count(), 1);
    let stderr = String::from_utf8(output.get_output().stderr.clone()).expect("UTF-8");
    assert!(stderr.contains("duplicate"), "got: {stderr}");
}

#[test]
fn a_version_is_reported() {
    lotus()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}
