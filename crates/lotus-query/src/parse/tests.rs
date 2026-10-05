// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for the CSV readers.
//!
//! Every fixture is a recorded answer from `QLever` or the WDQS, oddities
//! included: a padded formula, a bare year, a prefixed DOI, a row with no taxon,
//! an exact duplicate, and QIDs projected as bare integers because the query
//! strips the `Q` and hands back an `xsd:integer`.

// A test asserting on a fixture may panic when the fixture is wrong: that
// is the failure it is reporting, and the lint exists to keep library code
// free of panics on external input.
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
#![allow(clippy::format_collect)]

use super::*;
use lotus_model::WIKIDATA_STATEMENT_BASE;

/// Built from [`SELECT_COLUMNS`] rather than written out, because the fixtures
/// below it are positional. A hand-written header drifted from the projection
/// twice in one session and both times the failure was a column silently reading
/// as empty rather than an error.
fn header() -> String {
    crate::query::SELECT_COLUMNS.join(",")
}

fn csv(rows: &str) -> Vec<u8> {
    format!("{}\n{rows}", header()).into_bytes()
}

#[test]
fn the_iso_smiles_wins_over_the_connection_table_one() {
    let rows = parse_compounds_csv(
        &csv("Q1,L,IK,CC=CC,CC=CC,78.0,C6H6,Q10,T,Q100,Title,10.1/a,2021,http://x/S1\n"),
        10,
    )
    .expect("valid CSV");
    assert_eq!(rows[0].smiles.as_deref(), Some("CC=CC"));
}

#[test]
fn the_connection_table_smiles_is_used_when_there_is_no_iso_form() {
    let rows = parse_compounds_csv(
        &csv("Q1,L,IK,CC=CC,,78.0,C6H6,Q10,T,Q100,Title,10.1/a,2021,\n"),
        10,
    )
    .expect("valid CSV");
    assert_eq!(rows[0].smiles.as_deref(), Some("CC=CC"));
}

#[test]
fn a_doi_prefix_is_stripped_but_a_reference_title_is_not() {
    let rows = parse_compounds_csv(
        &csv("Q1,L,IK,CC=CC,,78.0,C6H6,Q10,T,Q100,,https://doi.org/10.1/B,10.1/B,2021,\n"),
        10,
    )
    .expect("valid CSV");
    assert_eq!(rows[0].ref_title.as_deref(), Some("https://doi.org/10.1/B"));
    assert_eq!(rows[0].ref_doi.as_deref(), Some("10.1/B"));
}

#[test]
fn a_publication_date_contributes_only_its_year() {
    for (input, expected) in [("2021-04-23T00:00:00Z", Some(2021)), ("2019", Some(2019))] {
        let rows = parse_compounds_csv(
            &csv(&format!(
                "Q1,L,IK,CC=CC,,78.0,C6H6,Q10,T,Q100,,T,10.1/a,{input},\n"
            )),
            10,
        )
        .expect("valid CSV");
        assert_eq!(rows[0].pub_year, expected, "input {input}");
    }
}

#[test]
fn a_row_without_a_compound_id_is_not_a_result() {
    let rows = parse_compounds_csv(
        &csv(",L,IK,CC=CC,,78.0,C6H6,Q10,T,Q100,T,10.1/a,2021,\n"),
        10,
    )
    .expect("valid CSV");
    assert_eq!(rows.len(), 0, "expected no entries");
}

#[test]
fn a_row_missing_several_columns_still_parses() {
    let rows = parse_compounds_csv(&csv("Q1,L\n"), 10).expect("valid CSV");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].taxon_qid.len(), 0, "expected no entries");
    assert!(rows[0].smiles.is_none());
}

#[test]
fn a_statement_uri_loses_its_prefix() {
    let uri = format!("{WIKIDATA_STATEMENT_BASE}S1");
    // Positional, so it has to track SELECT_COLUMNS: compound, label, inchikey,
    // smiles_conn, smiles_iso, mass, formula, taxon, taxon_name, ref_qid,
    // ref_node, ref_title, ref_doi, ref_date, statement_id.
    let rows = parse_compounds_csv(
        &csv(&format!(
            "Q1,L,IK,CC=CC,,78.0,C6H6,Q10,T,Q100,,T,10.1/a,2021,{uri}\n"
        )),
        10,
    )
    .expect("valid CSV");
    assert_eq!(rows[0].statement.as_deref(), Some("S1"));
}

#[test]
fn a_duplicate_triple_is_kept_once_and_counted_once_each_way() {
    let row = "Q1,L,IK,CC=CC,,78.0,C6H6,Q10,T,Q100,T,10.1/a,2021,\n";
    let (rows, stats, capped) =
        parse_compounds_csv_capped(&csv(&format!("{row}{row}")), 10).expect("valid CSV");
    assert_eq!(rows.len(), 1);
    assert_eq!(stats.n_entries, 2, "raw rows");
    assert_eq!(stats.n_entries_unique, 1, "distinct triples");
    assert!(!capped);
}

#[test]
fn capping_hides_rows_but_not_counts() {
    // Five distinct compounds, so the cap hides rows rather than duplicates.
    let rows: String = (1..=5)
        .map(|i| format!("Q{i},L,IK,CC=CC,,78.0,C6H6,Q10,T,Q100,T,10.1/a,2021,\n"))
        .collect();
    let (kept, stats, capped) = parse_compounds_csv_capped(&csv(&rows), 2).expect("valid CSV");
    assert_eq!(kept.len(), 2);
    assert!(
        capped,
        "the caller must be able to say the table is partial"
    );
    assert_eq!(stats.n_entries, 5);
    assert_eq!(stats.n_compounds, 5);
}

#[test]
fn a_zero_unique_count_falls_back_to_the_entry_count() {
    let bytes = b"n_entries,n_entries_unique,n_compounds,n_taxa,n_references\n10,0,3,2,4\n";
    let stats = parse_counts_csv(bytes).expect("valid CSV");
    assert_eq!(stats.n_entries_unique, 10);
}

#[test]
fn a_count_query_with_no_row_is_an_error() {
    // Unlike a row payload, there is nothing to fall back on: a total
    // cannot be invented.
    let err = parse_counts_csv(b"n_entries\n").expect_err("header only");
    assert!(err.message().contains("count"), "{err}");
}

#[test]
fn taxon_rows_need_both_an_id_and_a_name() {
    let bytes = b"taxon,taxon_name\nhttp://www.wikidata.org/entity/Q1,A\nQ2,\n,Q3\n";
    let matches = parse_taxon_csv(bytes).expect("valid CSV");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].qid, "Q1");
}

#[test]
fn an_empty_payload_is_an_empty_result_not_an_error() {
    assert_eq!(
        parse_compounds_csv(b"", 10).expect("empty is valid").len(),
        0,
        "an empty document yields no rows, and is not an error"
    );
}

// ── The taxon and compound lookup readers ─────────────────────────────────
//
// Both read a small result set whose columns are named, not positional, and
// both are looked up by name so that a query gaining or reordering a projection
// does not silently shift every field. These fixtures put the wanted columns
// somewhere other than first on purpose: a reader that took the first column it
// recognised, or the wrong one of several, would still produce rows here and only
// the values would be wrong.

#[test]
fn a_taxon_row_carries_which_property_answered() {
    // The notice about a common name is only worth showing when the endpoint
    // actually said so, so the token has to survive parsing rather than being
    // normalised away into "scientific".
    let bytes = b"taxon,taxon_name,matched_by\nQ1,bitterwort,common\n";
    let matches = parse_taxon_csv(bytes).expect("valid CSV");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].source, TaxonNameSource::Common);
}

#[test]
fn a_taxon_row_without_the_token_is_a_scientific_name() {
    // The query binds the token, so a row without one is an older or hand-made
    // fixture. Scientific is the safe reading: it is the claim that needs no
    // caveat.
    let bytes = b"taxon,taxon_name\nQ1,Gentiana lutea\n";
    let matches = parse_taxon_csv(bytes).expect("valid CSV");
    assert_eq!(matches[0].source, TaxonNameSource::Scientific);
}

#[test]
fn an_unrecognised_token_is_a_scientific_name() {
    let bytes = b"taxon,taxon_name,matched_by\nQ1,x,vernacular\n";
    let matches = parse_taxon_csv(bytes).expect("valid CSV");
    assert_eq!(
        matches[0].source,
        TaxonNameSource::Scientific,
        "only the one token the query binds may claim a common name"
    );
}

#[test]
fn taxon_columns_are_found_by_name_and_not_by_position() {
    // `taxon_name` is second and `taxon` third, so a positional reader would
    // swap them and report the wrong name against the wrong item.
    let bytes = b"taxon_name,matched_by,taxon\nGentiana lutea,scientific,Q1\n";
    let matches = parse_taxon_csv(bytes).expect("valid CSV");
    assert_eq!(matches[0].qid, "Q1");
    assert_eq!(matches[0].name, "Gentiana lutea");
}

#[test]
fn a_compound_lookup_row_carries_its_id_label_and_structure() {
    // The structure is what the two IDSM modes hand to the service, so a reader
    // that dropped it would leave a resolved compound with nothing to search.
    let bytes =
        b"compound_qid,compound_label,canonical_smiles\n18216,aspirin,CC(=O)OC1=CC=CC=C1C(=O)O\n";
    let matches = parse_compound_lookup_csv(bytes).expect("valid CSV");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].qid, "Q18216", "the bare integer regains its Q");
    assert_eq!(matches[0].label, "aspirin");
    assert_eq!(matches[0].canonical_smiles, "CC(=O)OC1=CC=CC=C1C(=O)O");
}

#[test]
fn compound_lookup_columns_are_found_by_name_and_not_by_position() {
    // Same reason as the taxon reader: three named columns, none of them first.
    let bytes = b"canonical_smiles,compound_qid,compound_label\nCC(=O)O,18216,aspirin\n";
    let matches = parse_compound_lookup_csv(bytes).expect("valid CSV");
    assert_eq!(matches[0].qid, "Q18216");
    assert_eq!(matches[0].label, "aspirin");
    assert_eq!(matches[0].canonical_smiles, "CC(=O)O");
}

#[test]
fn a_compound_row_with_no_id_is_not_a_match() {
    // A row the endpoint could not project an item for. Counting it would put a
    // compound in the candidate list that has no identity to search.
    let bytes = b"compound_qid,compound_label,canonical_smiles\n,orphan,\nQ18216,aspirin,CC(=O)O\n";
    let matches = parse_compound_lookup_csv(bytes).expect("valid CSV");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].qid, "Q18216");
}

#[test]
fn a_compound_with_no_label_or_structure_still_parses() {
    // Both are `OPTIONAL` in the query, so both are legitimately absent. The row
    // is still a match, and the caller falls back rather than dropping the
    // compound.
    let bytes = b"compound_qid,compound_label,canonical_smiles\n18216,,\n";
    let matches = parse_compound_lookup_csv(bytes).expect("valid CSV");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].label, "");
    assert_eq!(matches[0].canonical_smiles, "");
}

#[test]
fn a_lookup_payload_with_none_of_the_columns_is_no_rows() {
    // The reader is `flexible(true)` and finds each column by name, so bytes
    // that are not a lookup result parse to nothing rather than to an error.
    let bytes = b"something,else\n1,2\n";
    assert_eq!(
        parse_compound_lookup_csv(bytes).expect("valid CSV").len(),
        0,
        "no recognised column, so nothing to read"
    );
    assert_eq!(parse_taxon_csv(bytes).expect("valid CSV").len(), 0);
}

// ── Reading a payload that is not a result set ──────────────────────────────

#[test]
fn a_header_with_no_recognisable_columns_yields_no_rows() {
    // Documenting what this does, which is not what a `# Errors` section would
    // say it does. The CSV reader is built `flexible(true)` and the column
    // detector returns `None` for each column it cannot place, so bytes that are
    // not a result set at all parse to zero rows rather than to an error.
    //
    // That is the right call for a truncated or partially-typed export, and the
    // wrong one for a file that is not an export: the caller cannot tell "this
    // database has no compounds" from "this file is not a database".
    //
    // The streaming reader takes the opposite position on the same input --
    // `CsvColumnarReader::finish` refuses a payload it cannot recognise -- because
    // it is the path that holds a whole result set and would otherwise build an
    // empty one that reads as "the search matched nothing".
    let bytes = b"\0\0\0not a result set at all\n".to_vec();
    let (rows, _, capped) = parse_compounds_csv_capped(&bytes, 10).expect("reads");
    assert_eq!(rows.len(), 0, "expected no entries");
    assert!(!capped);
}

// ── The reference lookup reader ──────────────────────────────────────────────

#[test]
fn a_reference_lookup_reads_the_qid_out_of_the_item_uri() {
    // The query projects `?ref` as a URI, so the reader has to take the local name
    // off `http://www.wikidata.org/entity/` before it is a QID.
    let csv = b"ref\nhttp://www.wikidata.org/entity/Q23118\n";
    let matches = parse_reference_lookup_csv(csv).expect("reads");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].qid, "Q23118");
}

#[test]
fn a_reference_lookup_reads_every_row_in_order() {
    // Not the first row only. The resolver takes the first match, so the parser
    // has to hand them all over rather than stopping early: a DOI scan against
    // the scholarly subgraph can answer with more than one item.
    let csv = b"ref\n\
        http://www.wikidata.org/entity/Q23118\n\
        http://www.wikidata.org/entity/Q42\n";
    let matches = parse_reference_lookup_csv(csv).expect("reads");
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].qid, "Q23118");
    assert_eq!(matches[1].qid, "Q42");
}

#[test]
fn an_empty_reference_lookup_is_an_answer_rather_than_a_parse_error() {
    // "Wikidata does not have this reference" arrives as zero rows under a
    // perfectly good header, and it has to read as no match -- that is what the
    // resolver turns into "not found", as distinct from a transport failure that
    // would send the reader off to retry instead of to correct their DOI.
    let with_no_rows = parse_reference_lookup_csv(b"ref\n").expect("reads");
    assert_eq!(with_no_rows.len(), 0, "a header and no rows is no match");
    let with_no_header_rows = parse_reference_lookup_csv(b"ref").expect("reads");
    assert_eq!(with_no_header_rows.len(), 0, "a bare header is no match");
}

#[test]
fn a_reference_row_with_no_usable_uri_is_dropped() {
    // An empty or unparsable cell is not a reference, and offering it as one
    // would put an empty QID into the `VALUES` that constrains the search.
    let csv = b"ref\n\nnot-a-uri\nhttp://www.wikidata.org/entity/Q23118\n";
    let matches = parse_reference_lookup_csv(csv).expect("reads");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].qid, "Q23118");
}

#[test]
fn a_reference_lookup_tolerates_a_padded_header_and_extra_columns() {
    // `?ref` comes back padded, or renamed by whichever service answered, and the
    // scholarly subgraph projects columns the main one does not. Neither should
    // cost the QID.
    let csv = b" ref ,other\n  http://www.wikidata.org/entity/Q23118  ,ignored\n";
    let matches = parse_reference_lookup_csv(csv).expect("reads");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].qid, "Q23118");
}

#[test]
fn a_reference_lookup_without_a_ref_column_is_an_empty_answer() {
    // Nothing to read the QID from, so no rows. An error would be right only if a
    // missing column meant a broken query, and it does not: it is a lookup that
    // matched nothing, wearing a header that is not the one we asked for.
    let matches =
        parse_reference_lookup_csv(b"something_else\nhttp://www.wikidata.org/entity/Q1\n")
            .expect("reads");
    assert_eq!(matches.len(), 0, "no ref column means nothing to read");
}
