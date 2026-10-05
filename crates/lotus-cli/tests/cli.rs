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
    // `*` rather than a name: a name has to be looked up, which `--explain`
    // refuses, and this test is about the shape of the printed query rather than
    // about taxon resolution -- which `explain_prints_the_query_for_the_taxon_it_
    // was_given` covers.
    let output = lotus()
        .args(["search", "--taxon", "*", "--explain"])
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

/// `--explain` prints the query for the request actually being made.
///
/// This test once asserted the opposite. `--explain` passed `None` as the
/// resolved taxon to stay offline, so `--taxon Q21754 --explain` printed the
/// *no-taxon* query -- no `P171*` anywhere -- and said nothing. A flag read when a
/// result surprises someone cannot hand back a different request's query.
///
/// Offline is kept, and is why the fix passes a bare QID through instead of
/// resolving: a QID needs no lookup, so only the name case has to refuse.
#[test]
fn explain_prints_the_query_for_the_taxon_it_was_given() {
    // A bare QID needs no resolution, so the printed query is the real one.
    let output = lotus()
        .args(["search", "--taxon", "Q21754", "--explain"])
        .assert()
        .success();
    let query = String::from_utf8(output.get_output().stdout.clone()).expect("UTF-8");
    assert!(
        query.contains("wd:Q21754"),
        "a QID needs no lookup, so the ancestry filter must be present:\n{query}"
    );
    assert!(
        query.contains("P171"),
        "and it is a descendant filter, not a bare equality:\n{query}"
    );

    // An empty box constrains nothing, so the occurrence is optional and the
    // compounds nobody has tied to an organism are reachable.
    let output = lotus().args(["search", "--explain"]).assert().success();
    let query = String::from_utf8(output.get_output().stdout.clone()).expect("UTF-8");
    let p703 = query.find("?c p:P703").expect("P703 is bound");
    let optional = query[..p703].rfind("OPTIONAL").is_some_and(|at| {
        query[at + "OPTIONAL".len()..p703]
            .chars()
            .all(|c| c.is_whitespace() || c == '{')
    });
    assert!(
        optional,
        "an empty taxon box must reach the untaxonomised compounds:\n{query}"
    );

    // `*` asks for what has been reported, so it keeps requiring the occurrence.
    let output = lotus()
        .args(["search", "--taxon", "*", "--explain"])
        .assert()
        .success();
    let query = String::from_utf8(output.get_output().stdout.clone()).expect("UTF-8");
    let p703 = query.find("?c p:P703").expect("P703 is bound");
    let optional = query[..p703].rfind("OPTIONAL").is_some_and(|at| {
        query[at + "OPTIONAL".len()..p703]
            .chars()
            .all(|c| c.is_whitespace() || c == '{')
    });
    assert!(
        !optional,
        "`*` is not an empty box; it requires P703:\n{query}"
    );
}

/// A taxon *name* refuses under `--explain` rather than printing the wrong query.
///
/// Turning a name into a QID is the network call the flag exists to avoid, so
/// there is no correct query to print. Failing with the QID form named is the
/// honest answer; printing the unfiltered query is the bug this replaced.
#[test]
fn explain_refuses_a_taxon_name_instead_of_printing_the_unfiltered_query() {
    lotus()
        .args(["search", "--taxon", "Gentiana lutea", "--explain"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not resolve taxon names"))
        .stderr(predicate::str::contains("Q21754"))
        .stdout(predicate::str::contains("PREFIX").not());
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
        .args([
            "search",
            "--structure",
            "c1ccccc1",
            "--structure-search",
            "substructure",
            "--explain",
        ])
        .assert()
        .success();
    let query = String::from_utf8(sub.get_output().stdout.clone()).expect("UTF-8");
    assert!(query.contains("sachem:substructureSearch"));
    assert!(!query.contains("sachem:cutoff"));
}

#[test]
fn a_bare_structure_search_asks_for_the_same_molecule() {
    // The default is similarity at 1.0, which is "this compound" for a structure
    // no Wikidata item has. Substructure has to be asked for.
    let output = lotus()
        .args(["search", "--structure", "c1ccccc1", "--explain"])
        .assert()
        .success();
    let query = String::from_utf8(output.get_output().stdout.clone()).expect("UTF-8");
    assert!(query.contains("sachem:similarCompoundSearch"));
    assert!(
        query.contains(r#"sachem:cutoff "1"^^xsd:double"#),
        "1.0 is written as 1; the default must be an exact-fingerprint match: {query}"
    );
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
fn curate_says_that_nothing_was_submitted() {
    let tsv = "name\tsmiles\ttaxon\tdoi\nQuercetin\tCCO\tGentiana lutea\t10.1/a\n";
    let output = lotus()
        .args(["curate", "-", "--offline"])
        .write_stdin(tsv)
        .assert()
        .success();
    let stderr = String::from_utf8(output.get_output().stderr.clone()).expect("UTF-8");
    assert!(
        stderr.contains("nothing has been submitted"),
        "the reminder is the only thing standing between this output and a \
         reader believing it reached Wikidata: {stderr}"
    );
}

#[test]
fn curate_quiet_suppresses_the_reminder_and_nothing_else() {
    // `--quiet` silences the reminder. It must not change the statements, or it
    // would be a second, undocumented way to run the command.
    let tsv = "name\tsmiles\ttaxon\tdoi\nQuercetin\tCCO\tGentiana lutea\t10.1/a\n";
    let loud = lotus()
        .args(["curate", "-", "--offline"])
        .write_stdin(tsv)
        .assert()
        .success();
    let quiet = lotus()
        .args(["curate", "-", "--offline", "--quiet"])
        .write_stdin(tsv.to_string())
        .assert()
        .success();
    assert_eq!(
        loud.get_output().stdout,
        quiet.get_output().stdout,
        "--quiet changed the output"
    );
    assert!(
        String::from_utf8(quiet.get_output().stderr.clone())
            .expect("UTF-8")
            .is_empty(),
        "--quiet left something on stderr"
    );
}

#[test]
fn curate_writes_the_occurrence_and_the_reference_as_separate_statements() {
    // They are separate because a `QuickStatements` run stops at the first
    // failure: an occurrence pointing at a taxon nobody has created would take
    // down the compound statement in the same block.
    let tsv = "name\tsmiles\ttaxon\tdoi\nQuercetin\tCCO\tGentiana lutea\t10.1/a\n";
    let output = lotus()
        .args(["curate", "-", "--offline"])
        .write_stdin(tsv)
        .assert()
        .success();
    let stderr = String::from_utf8(output.get_output().stderr.clone()).expect("UTF-8");
    assert!(stderr.contains("nothing has been submitted"), "{stderr}");
    // `json` is the format that carries the statements; `table` appends them
    // after the rows and `jsonl` is one row per line with no room for them.
    let output = lotus()
        .args(["curate", "-", "--offline", "--format", "json"])
        .write_stdin("name\tsmiles\ttaxon\tdoi\nQuercetin\tCCO\tGentiana lutea\t10.1/a\n")
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).expect("UTF-8");
    assert!(stdout.contains("P233"), "canonical SMILES: {stdout}");
    assert!(stdout.contains("P2017"), "isomeric SMILES: {stdout}");
    assert!(stdout.contains("P703"), "occurrence: {stdout}");
    assert!(stdout.contains("P248"), "reference: {stdout}");
}

#[test]
fn curate_escapes_a_quote_in_a_name() {
    // An unescaped quote ends the scalar early, and the rest of the value is
    // then read as new properties -- so a compound called `Say "hi"` would emit
    // a statement that writes `hi` as a property name.
    let tsv = "name\tsmiles\nSay \"hi\"\tCCO\n";
    let output = lotus()
        .args(["curate", "-", "--offline", "--format", "json"])
        .write_stdin(tsv)
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).expect("UTF-8");
    assert!(stdout.contains(r#"Say \"hi\""#), "not escaped: {stdout}");
}

#[test]
fn curate_reads_stdin_and_ignores_a_row_with_no_smiles() {
    let tsv =
        "name\tsmiles\ttaxon\tdoi\nQuercetin\tCCO\tGentiana lutea\t10.1/a\nNoSmiles\t\tX\t10.1/b\n";
    let output = lotus()
        .args(["curate", "-", "--offline", "--format", "tsv"])
        .write_stdin(tsv)
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).expect("UTF-8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "name\tsmiles\ttaxon\tdoi\twikidata_qid\tstatus");
    assert_eq!(lines.len(), 2, "the incomplete row was dropped: {stdout}");
    assert!(lines[1].contains("Quercetin"));
}

#[test]
fn curate_reports_a_missing_column_rather_than_reading_the_wrong_ones() {
    let output = lotus()
        .args(["curate", "-", "--offline"])
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
        .args(["curate", "-", "--offline", "--format", "jsonl"])
        .write_stdin(tsv)
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).expect("UTF-8");
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 2);
    for line in lines {
        let value: serde_json::Value = serde_json::from_str(line).expect("one object per line");
        // The row carries its input, so a consumer can see what was asked as
        // well as what was found.
        assert!(value["input"]["name"].is_string(), "{line}");
    }
}

#[test]
fn curate_deduplicates_rows_that_name_the_same_finding() {
    // The same structure, taxon and DOI under two names is one finding: a
    // spreadsheet artefact, not two things to submit.
    let tsv = "name\tsmiles\ttaxon\tdoi\nQuercetin\tCCO\tGentiana lutea\t10.1/a\nAlso called quercetin\tcco\tGENTIANA LUTEA\t10.1/A\n";
    let output = lotus()
        .args(["curate", "-", "--offline", "--format", "jsonl"])
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

#[test]
fn an_offline_run_says_it_did_not_look_rather_than_saying_the_row_is_new() {
    // The distinction the whole command turns on. A row that was never checked
    // has not been shown to be absent from Wikidata, and reporting it as new is
    // how a curator submits a duplicate of something that has been there for
    // years.
    let tsv = "name\tsmiles\nQuercetin\tCCO\n";
    let output = lotus()
        .args(["curate", "-", "--offline", "--format", "tsv"])
        .write_stdin(tsv)
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).expect("UTF-8");

    assert!(stdout.contains("not_checked"), "got: {stdout}");
    assert!(
        !stdout.contains("new_compound"),
        "an unchecked row is not new: {stdout}"
    );
}

#[test]
fn an_offline_row_still_carries_a_readable_draft_of_the_statements() {
    // Being offline says what was not checked, not that there is nothing to
    // read. A curator wants to see the bundle; they want to know it is a draft.
    let tsv = "name\tsmiles\nQuercetin\tCCO\n";
    let output = lotus()
        .args(["curate", "-", "--offline", "--format", "json"])
        .write_stdin(tsv)
        .assert()
        .success();
    let value: serde_json::Value =
        serde_json::from_slice(&output.get_output().stdout).expect("valid JSON");

    assert_eq!(value["rows"][0]["status"], "NotChecked");
    assert!(
        value["rows"][0]["note"]
            .as_str()
            .is_some_and(|note| note.contains("not looked up")),
        "the row must say why: {value}",
    );
    assert!(
        value["statements"]
            .as_array()
            .is_some_and(|s| !s.is_empty()),
        "there is still a draft to read: {value}"
    );
}

#[test]
fn curate_reports_how_many_rows_it_was_not_able_to_check() {
    let tsv = "name\tsmiles\nA\tCCO\nB\tCCN\n";
    let output = lotus()
        .args(["curate", "-", "--offline"])
        .write_stdin(tsv)
        .assert()
        .success();
    let stderr = String::from_utf8(output.get_output().stderr.clone()).expect("UTF-8");
    assert!(stderr.contains("not looked up"), "got: {stderr}");
}
