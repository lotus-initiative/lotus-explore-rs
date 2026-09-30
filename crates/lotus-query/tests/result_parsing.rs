// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Reading a recorded `QLever` response.
//!
//! The fixtures are what the endpoint actually returns, oddities included: a
//! padded formula, a bare year, a prefixed DOI, a row with no taxon, and an
//! exact duplicate. Every downstream surface reads these types, so this is the
//! test that decides whether a search returns the right rows.

// The panic lints exist to keep library code free of panics on external input.
// A test that fails on a bad fixture is reporting, not panicking.
#![allow(unused_crate_dependencies)]
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use lotus_model::DatasetStats;
use lotus_query::{
    parse_compounds_csv, parse_compounds_csv_capped, parse_counts_csv, parse_taxon_csv,
};

fn fixture(name: &str) -> Vec<u8> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/");
    std::fs::read(format!("{path}{name}")).expect("the fixture is readable")
}

#[test]
fn a_duplicate_triple_appears_once() {
    // The fixture repeats Q1/Q16521/Q1000 verbatim on its last line.
    let rows = parse_compounds_csv(&fixture("compounds.csv"), 100).expect("valid CSV");
    assert_eq!(rows.len(), 3, "four rows, one of them an exact duplicate");
    assert_eq!(rows[0].compound_qid.as_ref(), "Q1");
    assert_eq!(rows[0].name.as_ref(), "Quercetin");
    assert_eq!(rows[0].taxon_qid.as_ref(), "Q16521");
    assert_eq!(rows[0].reference_qid.as_ref(), "Q1000");
    assert_eq!(rows[2].compound_qid.as_ref(), "Q3");
}

/// The QID projection: the query strips the `Q` and hands back an integer.
///
/// `compounds.csv` cannot catch this, because its QIDs are written `Q1` rather
/// than the `1` the live endpoint returns -- so the fixture passed while every
/// rendered link was wrong. This is the fixture in the shape it actually arrives.
const INTEGER_QIDS: &str = "compound,compoundLabel,compound_inchikey,taxon,taxon_name,ref_qid,ref_title,ref_doi,ref_date,statement\n\
16521,Quercetin,IIYFPWUAQGXNF,16521,Gentiana lutea,1000,An article,10.1/A,2021,http://www.wikidata.org/entity/statement/S1\n";

#[test]
fn a_qid_projected_as_an_integer_gets_its_prefix_back() {
    let rows = parse_compounds_csv(INTEGER_QIDS.as_bytes(), 100).expect("valid CSV");
    assert_eq!(rows[0].compound_qid.as_ref(), "Q16521");
    assert_eq!(rows[0].taxon_qid.as_ref(), "Q16521");
    assert_eq!(rows[0].reference_qid.as_ref(), "Q1000");
}

#[test]
fn an_absent_taxon_is_empty_rather_than_missing() {
    // A row can name a compound and a reference but no taxon; the QID is
    // then an empty string, not `None`, because the column is present.
    let rows = parse_compounds_csv(&fixture("compounds.csv"), 100).expect("valid CSV");
    let row = rows
        .iter()
        .find(|r| r.compound_qid.as_ref() == "Q3")
        .expect("Q3 is present");
    assert!(row.taxon_qid.is_empty());
    assert!(row.taxon_name.is_empty());
    assert_eq!(row.reference_qid.as_ref(), "Q1001");
}

#[test]
fn a_column_the_query_never_projected_is_none() {
    // The reverse of an empty value: a column absent from the header is
    // genuinely `None`, and the two cases must not be confused.
    let csv = b"compound,compoundLabel,taxon,ref_qid\nQ1,One,Q10,Q100\n";
    let rows = parse_compounds_csv(csv, 10).expect("valid CSV");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].taxon_qid.as_ref(),
        "Q10",
        "a present but empty cell"
    );
    assert!(
        rows[0].smiles.is_none(),
        "a column that was not projected at all"
    );
    assert!(rows[0].mass.is_none());
}

#[test]
fn the_isomeric_smiles_is_preferred() {
    let rows = parse_compounds_csv(&fixture("compounds.csv"), 100).expect("valid CSV");
    let row = rows
        .iter()
        .find(|r| r.compound_qid.as_ref() == "Q2")
        .expect("Q2 is present");
    // P2017 (isomeric) wins over P233 (connection table).
    assert_eq!(row.smiles.as_deref(), Some("CC1=CC=CC=C1"));
    assert_eq!(row.mass, Some(92.138));
    assert_eq!(row.formula.as_deref(), Some("C7H8"));
}

#[test]
fn a_padded_formula_is_trimmed() {
    let rows = parse_compounds_csv(&fixture("compounds.csv"), 100).expect("valid CSV");
    let row = rows
        .iter()
        .find(|r| r.compound_qid.as_ref() == "Q1")
        .expect("Q1 is present");
    // The source pads the formula to align the column.
    assert_eq!(row.formula.as_deref(), Some("C17H12O7"));
}

#[test]
fn a_doi_keeps_its_value_and_loses_its_prefix() {
    let rows = parse_compounds_csv(&fixture("compounds.csv"), 100).expect("valid CSV");
    let row = rows
        .iter()
        .find(|r| r.compound_qid.as_ref() == "Q3")
        .expect("Q3 is present");
    assert_eq!(row.ref_title.as_deref(), Some("Other article"));
    assert_eq!(row.ref_doi.as_deref(), Some("10.1/B"));
}

#[test]
fn a_publication_date_contributes_only_its_year() {
    let rows = parse_compounds_csv(&fixture("compounds.csv"), 100).expect("valid CSV");
    let dated = rows
        .iter()
        .find(|r| r.compound_qid.as_ref() == "Q1")
        .expect("Q1 is present");
    let bare = rows
        .iter()
        .find(|r| r.compound_qid.as_ref() == "Q3")
        .expect("Q3 is present");
    assert_eq!(dated.pub_year, Some(2021), "a full xsd:date");
    assert_eq!(bare.pub_year, Some(2019), "a bare year");
}

#[test]
fn a_statement_uri_loses_its_wikidata_prefix() {
    let rows = parse_compounds_csv(&fixture("compounds.csv"), 100).expect("valid CSV");
    assert_eq!(rows[0].statement.as_deref(), Some("S1"));
    assert_eq!(rows[2].statement.as_deref(), Some("S2"));
}

#[test]
fn the_count_query_agrees_with_a_local_count_on_the_whole_set() {
    // The endpoint's `COUNT` and the streaming reader's own tally are two
    // routes to the same numbers; if they drift, the totals a user sees drift
    // with them.
    let (_, streamed, _) =
        parse_compounds_csv_capped(&fixture("compounds.csv"), 100).expect("valid CSV");
    let reported = parse_counts_csv(&fixture("counts.csv")).expect("valid CSV");

    assert_eq!(streamed.n_entries, reported.n_entries);
    assert_eq!(streamed.n_entries_unique, reported.n_entries_unique);
    assert_eq!(streamed.n_compounds, reported.n_compounds);
    assert_eq!(streamed.n_taxa, reported.n_taxa);
    assert_eq!(streamed.n_references, reported.n_references);
}

#[test]
fn counting_local_rows_cannot_reproduce_the_raw_entry_count() {
    // A caller that only has deduplicated rows cannot recover `n_entries`: the
    // dedup happened during parsing. The counts query or the capped reader is
    // the only route to the true total.
    let rows = parse_compounds_csv(&fixture("compounds.csv"), 100).expect("valid CSV");
    let local = DatasetStats::from_deduplicated_entries(&rows);
    let reported = parse_counts_csv(&fixture("counts.csv")).expect("valid CSV");

    assert_eq!(local.n_entries, 3, "the deduplicated count");
    assert_eq!(reported.n_entries, 4, "the endpoint's count");
    assert_eq!(local.n_compounds, reported.n_compounds);
    assert_eq!(local.n_entries_unique, reported.n_entries_unique);
}

#[test]
fn capping_hides_rows_but_reports_the_whole_set() {
    let (rows, stats, capped) =
        parse_compounds_csv_capped(&fixture("compounds.csv"), 2).expect("valid CSV");
    assert_eq!(rows.len(), 2);
    assert!(
        capped,
        "the caller must be able to say the table is partial"
    );
    assert_eq!(stats.n_entries, 4, "the count still saw every row");
    assert_eq!(stats.n_entries_unique, 3);
    assert_eq!(
        stats.n_compounds, 3,
        "Q1, Q2 and Q3, not the two that were kept"
    );
}

#[test]
fn a_limit_above_the_row_count_is_not_reported_as_capped() {
    let (rows, _, capped) =
        parse_compounds_csv_capped(&fixture("compounds.csv"), 1000).expect("valid CSV");
    assert_eq!(rows.len(), 3);
    assert!(!capped);
}

#[test]
fn a_row_without_a_compound_id_is_not_a_result() {
    let csv = b"compound,compoundLabel,taxon,ref_qid\n,Q1,cmpd,,Q100\n";
    let rows = parse_compounds_csv(csv, 50).expect("valid CSV");
    assert!(rows.is_empty(), "a row with no QID is not a compound");
}

#[test]
fn a_payload_the_endpoint_mangled_still_parses() {
    // `flexible(true)` plus per-field defaulting means an odd payload degrades
    // to emptier columns. A search that has usable rows should not be discarded
    // because one cell is unparsable.
    for (name, payload) in [
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
        assert!(
            parse_compounds_csv(payload, 10).is_ok(),
            "{name} should parse rather than fail"
        );
    }
}

#[test]
fn an_empty_payload_is_an_empty_result_rather_than_an_error() {
    assert!(
        parse_compounds_csv(b"", 10)
            .expect("empty is not malformed")
            .is_empty()
    );
}

#[test]
fn a_taxon_payload_accepts_every_shape_a_qid_arrives_in() {
    // A full URI, and the typed integer literal the QID projection produces.
    let matches = parse_taxon_csv(&fixture("taxon.csv")).expect("valid CSV");
    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].qid, "Q16521");
    assert_eq!(matches[0].name, "Gentiana lutea");
    assert_eq!(matches[1].qid, "Q456");
    assert_eq!(matches[1].name, "Gentiana lutea");
    assert_eq!(matches[2].qid, "Q999");
}

#[test]
fn a_taxon_row_needs_both_an_id_and_a_name() {
    let csv = b"taxon,taxon_name\nQ1,A\nQ2,\n,Q3\n";
    let matches = parse_taxon_csv(csv).expect("valid CSV");
    assert_eq!(matches.len(), 1, "a half-filled row is not a match");
}

#[test]
fn a_header_only_taxon_payload_is_no_matches() {
    let matches = parse_taxon_csv(b"taxon,taxon_name\n").expect("valid CSV");
    assert!(matches.is_empty());
}

#[test]
fn a_zero_unique_count_falls_back_to_the_entry_count() {
    // The endpoint reports 0 rather than erroring when a count subquery is
    // skipped. Reporting 0 would understate the result set; the raw count is
    // the lesser wrong answer.
    let csv = b"n_entries,n_entries_unique,n_compounds,n_taxa,n_references\n10,0,3,2,4\n";
    let stats = parse_counts_csv(csv).expect("valid CSV");
    assert_eq!(stats.n_entries, 10);
    assert_eq!(stats.n_entries_unique, 10);
}

#[test]
fn a_count_payload_with_no_row_is_an_error() {
    // Unlike a row payload, a missing total cannot be invented.
    let err = parse_counts_csv(b"n_entries\n").expect_err("header only");
    assert!(err.to_string().contains("no row"), "got: {err}");
    assert!(parse_counts_csv(b"").is_err());
}

#[test]
fn a_count_column_the_endpoint_omitted_reads_as_zero() {
    let csv = b"n_entries,n_compounds\n7,2\n";
    let stats = parse_counts_csv(csv).expect("valid CSV");
    assert_eq!(stats.n_entries, 7);
    assert_eq!(
        stats.n_entries_unique, 7,
        "and so is folded into the unique count"
    );
    assert_eq!(stats.n_taxa, 0);
}
