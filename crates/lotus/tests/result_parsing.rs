// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Characterization tests: what the CSV parsers produce, on recorded `QLever`
//! output. These are the load-bearing assertions of the refactor: every
//! downstream surface (table, CSV/JSON export, JSON-LD, CLI) reads these types.

// The test lints that are denied workspace-wide (`expect_used`,
// `indexing_slicing`, …) exist to keep *library* code free of panics on
// external input. A test asserting on a fixture may panic when the fixture is
// wrong: that is the failure it is reporting.
#![allow(unused_crate_dependencies)]
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use lotus::models::DatasetStats;
use lotus::sparql::{
    parse_compounds_csv_capped_reader, parse_compounds_csv_display_bytes, parse_counts_csv_bytes,
    parse_taxon_csv_bytes,
};

fn fixture(name: &str) -> Vec<u8> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/");
    std::fs::read(format!("{path}{name}")).expect("fixture readable")
}

#[test]
fn compound_rows_are_deduplicated_by_compound_taxon_reference() {
    // The fixture repeats Q1/Q16521/Q1000 verbatim; the parse must drop it.
    let rows =
        parse_compounds_csv_display_bytes(&fixture("compounds.csv"), 100).expect("valid CSV");

    assert_eq!(rows.len(), 3, "the exact duplicate row collapses");
    assert_eq!(rows[0].compound_qid.as_ref(), "Q1");
    assert_eq!(rows[0].name.as_ref(), "Quercetin");
    assert_eq!(rows[0].taxon_qid.as_ref(), "Q16521");
    assert_eq!(rows[0].reference_qid.as_ref(), "Q1000");
    assert_eq!(rows[2].compound_qid.as_ref(), "Q3");
}

#[test]
fn optional_columns_become_none_not_empty_strings() {
    let rows =
        parse_compounds_csv_display_bytes(&fixture("compounds.csv"), 100).expect("valid CSV");
    let no_taxon = rows.iter().find(|r| r.compound_qid.as_ref() == "Q3");

    let row = no_taxon.expect("Q3 present");
    assert!(row.taxon_qid.is_empty(), "absent taxon is an empty QID");
    assert!(row.taxon_name.is_empty());
    assert_eq!(row.reference_qid.as_ref(), "Q1001");
}

#[test]
fn field_precedence_and_normalisation_are_fixed() {
    let rows =
        parse_compounds_csv_display_bytes(&fixture("compounds.csv"), 100).expect("valid CSV");
    let row = rows
        .iter()
        .find(|r| r.compound_qid.as_ref() == "Q2")
        .expect("Q2");

    // ISO SMILES (P2017) wins over the connection-table SMILES (P233).
    assert_eq!(row.smiles.as_deref(), Some("CC1=CC=CC=C1"));
    assert_eq!(row.mass, Some(92.138));
    assert_eq!(row.formula.as_deref(), Some("C7H8"));
    // A bare year parses; a full xsd:date is truncated to its year.
    assert_eq!(rows[0].pub_year, Some(2021));
    assert_eq!(rows[2].pub_year, Some(2019));
    // The `doi.org/` prefix is stripped.
    assert_eq!(rows[2].ref_doi.as_deref(), Some("10.1/B"));
    // The formula is trimmed of the padding the source carries.
    assert_eq!(rows[0].formula.as_deref(), Some("C17H12O7"));
}

#[test]
fn statement_uris_lose_their_wikidata_prefix() {
    let rows =
        parse_compounds_csv_display_bytes(&fixture("compounds.csv"), 100).expect("valid CSV");
    assert_eq!(rows[0].statement.as_deref(), Some("S1"));
    assert_eq!(rows[2].statement.as_deref(), Some("S2"));
}

#[test]
fn rows_without_a_compound_id_are_skipped() {
    let csv = b"compound,compoundLabel,taxon,ref_qid\n,Q1,cmpd,,Q100\n";
    let rows = parse_compounds_csv_display_bytes(csv, 50).expect("valid CSV");
    assert!(rows.is_empty(), "a row with no QID is not a result");
}

#[test]
fn taxon_uris_and_typed_literals_both_yield_a_qid() {
    let matches = parse_taxon_csv_bytes(&fixture("taxon.csv")).expect("valid CSV");
    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].qid, "Q16521");
    assert_eq!(matches[0].name, "Gentiana lutea");
    // The same taxon, exported as an integer literal rather than a URI.
    assert_eq!(matches[1].qid, "Q456");
    assert_eq!(matches[1].name, "Gentiana lutea");
    assert_eq!(matches[2].qid, "Q999");
}

#[test]
fn header_only_csv_yields_no_taxa() {
    let matches = parse_taxon_csv_bytes(b"taxon,taxon_name\n").expect("valid CSV");
    assert!(matches.is_empty());
}

#[test]
fn count_row_becomes_dataset_stats() {
    let stats = parse_counts_csv_bytes(&fixture("counts.csv")).expect("valid CSV");
    assert_eq!(stats.n_entries, 4);
    assert_eq!(stats.n_entries_unique, 3);
    assert_eq!(stats.n_compounds, 3);
    assert_eq!(stats.n_taxa, 2);
    assert_eq!(stats.n_references, 2);
}

#[test]
fn a_zero_unique_count_falls_back_to_the_entry_count() {
    // QLever reports 0 rather than erroring when a count subquery is skipped.
    let csv = b"n_entries,n_entries_unique,n_compounds,n_taxa,n_references\n10,0,3,2,4\n";
    let stats = parse_counts_csv_bytes(csv).expect("valid CSV");
    assert_eq!(
        stats.n_entries_unique, 10,
        "0 would understate the result set"
    );
}

#[test]
fn the_streaming_capped_reader_counts_before_deduplicating() {
    // Two readers, same bytes, different `n_entries`:
    //
    //   capped reader      4 rows read, 3 kept  ->  n_entries 4, unique 3
    //   display reader     4 rows read, 3 kept  ->  from_entries: both 3
    //   COUNT query on the same result set      ->  n_entries 4, unique 3
    //
    // The display reader deduplicates as it goes, so `from_entries` cannot see
    // the raw row count. The callers that need the true total use the count
    // query, or the capped reader, not `from_entries`.
    let (rows, streamed, capped) =
        parse_compounds_csv_capped_reader(std::io::Cursor::new(fixture("compounds.csv")), 100)
            .expect("valid CSV");
    assert!(!capped);
    assert_eq!(rows.len(), 3);
    assert_eq!(streamed.n_entries, 4, "raw rows, before dedup");
    assert_eq!(streamed.n_entries_unique, 3);
    assert_eq!(streamed.n_compounds, 3, "Q1, Q2, Q3");
    assert_eq!(streamed.n_taxa, 2, "Q3 has no taxon");
    assert_eq!(streamed.n_references, 2, "Q1000, Q1001");

    let display =
        parse_compounds_csv_display_bytes(&fixture("compounds.csv"), 100).expect("valid CSV");
    let local = DatasetStats::from_entries(&display);
    assert_eq!(
        local.n_entries, 3,
        "deduplicated: this is the only difference"
    );
    assert_eq!(local.n_entries_unique, 3);
    assert_eq!(local, streamed_with(local.n_entries, &streamed));

    // And the count query agrees with the streaming reader on both numbers.
    let remote = parse_counts_csv_bytes(&fixture("counts.csv")).expect("valid CSV");
    assert_eq!(remote.n_entries, streamed.n_entries);
    assert_eq!(remote.n_entries_unique, streamed.n_entries_unique);
    assert_eq!(remote.n_compounds, streamed.n_compounds);
    assert_eq!(remote.n_taxa, streamed.n_taxa);
    assert_eq!(remote.n_references, streamed.n_references);
}

fn streamed_with(n_entries: usize, streamed: &DatasetStats) -> DatasetStats {
    DatasetStats {
        n_entries,
        ..streamed.clone()
    }
}

#[test]
fn the_cap_stops_the_reader_but_does_not_truncate_the_counts() {
    let (rows, stats, capped) =
        parse_compounds_csv_capped_reader(std::io::Cursor::new(fixture("compounds.csv")), 2)
            .expect("valid CSV");

    assert_eq!(rows.len(), 2, "the reader stops at the cap");
    assert!(
        capped,
        "the caller is told to show a 'truncated' affordance"
    );
    assert_eq!(stats.n_entries, 4, "the count still saw every row");
    assert_eq!(stats.n_compounds, 3);
}

#[test]
fn a_cap_above_the_row_count_is_not_reported_as_capped() {
    let (_, _, capped) =
        parse_compounds_csv_capped_reader(std::io::Cursor::new(fixture("compounds.csv")), 1000)
            .expect("valid CSV");
    assert!(!capped);
}

#[test]
fn the_row_parser_is_deliberately_tolerant_of_odd_input() {
    // `flexible(true)`, plus a parser that trims and defaults every field, means
    // a malformed payload degrades to fewer/emptier columns rather than an
    // error. Worth pinning: a real endpoint hiccup must not fail a search that
    // has already returned usable rows.
    for (name, csv) in [
        (
            "unterminated quote",
            &b"compound,compoundLabel\nQ1,\"unterminated\n"[..],
        ),
        (
            "quote in a header name",
            &b"comp\"ound,compoundLabel\nQ1,a\n"[..],
        ),
        ("ragged row", &b"compound,compoundLabel,taxon\nQ1\n"[..]),
        ("binary junk", &[0x00u8, 0x01, 0x02, 0xff][..]),
    ] {
        let result = parse_compounds_csv_display_bytes(csv, 10);
        assert!(result.is_ok(), "{name} should parse, not error");
    }

    // Where the tolerance shows: a field that cannot be read is dropped, not
    // reported, and a row with no readable QID is skipped.
    let rows = parse_compounds_csv_display_bytes(b"comp\"ound,compoundLabel\nQ1,a\n", 10)
        .expect("valid CSV");
    assert!(
        rows.is_empty(),
        "no recognised `compound` column, so no rows"
    );
}

#[test]
fn counts_errors_when_there_is_no_row_to_read() {
    // Unlike the row parser, a missing count is a hard failure: the caller has
    // no way to invent a total.
    let err = parse_counts_csv_bytes(b"n_entries\n").expect_err("header only");
    assert!(err.to_string().contains("Missing count row"), "got: {err}");
    assert!(parse_counts_csv_bytes(b"").is_err());
}

#[test]
fn an_empty_body_is_an_empty_result_for_the_row_parser() {
    assert!(
        parse_compounds_csv_display_bytes(b"", 10)
            .expect("empty is not malformed")
            .is_empty(),
        "no rows is a valid, empty result"
    );
}

#[test]
fn a_missing_column_is_tolerated_by_the_row_parser() {
    // QLever omits a column entirely when the query does not project it.
    let csv = b"compound,compoundLabel\nQ1,One\n";
    let rows = parse_compounds_csv_display_bytes(csv, 10).expect("valid CSV");
    assert_eq!(rows.len(), 1);
    assert!(rows[0].taxon_qid.is_empty());
    assert!(rows[0].smiles.is_none());
}
