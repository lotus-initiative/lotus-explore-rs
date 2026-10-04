// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The two paths into a result set must produce the same bytes.
//!
//! There are two ways a [`ColumnarResultSet`] gets built. `from_entries` takes
//! values a caller already holds; the streaming reader folds an endpoint's CSV a
//! chunk at a time. Production uses the second and every test in the crate used
//! the first, which means the streaming path -- the one with a chunk-boundary
//! bug, a column-name typo and a truncation check in it -- was compared against
//! nothing but itself.
//!
//! So this compares the two against each other, and it compares what comes out
//! rather than what goes in: **the same rows, through both paths, must render
//! byte-identical CSV, JSON and Turtle**, and read back as the same entries and
//! the same statistics. A change to the parser, the projection, the store or the
//! exporter that alters what a reader sees fails here.
//!
//! Deliberately not a golden-file test. A recorded file is a copy that has to be
//! regenerated deliberately, and regenerating it is exactly the moment someone
//! decides the new output is fine. This compares two live paths, so there is
//! nothing to bless: the question is never "is this the right output" but "are
//! these two the same output", which is the question a refactor is allowed to ask.

#![allow(unused_crate_dependencies, clippy::expect_used, clippy::panic)]

use lotus_model::{ColumnarResultSet, CompoundEntry};
use lotus_query::{CsvColumnarReader, ExportFormat, RowExporter, SELECT_COLUMNS};
use std::sync::Arc;

fn arc(text: &str) -> Arc<str> {
    Arc::from(text)
}

/// A row with every column filled, including the ones whose values are awkward
/// to serialise: a comma in a DOI, a quote and a newline in a title.
fn full_row() -> CompoundEntry {
    CompoundEntry {
        compound_qid: arc("Q3613679"),
        name: arc("quercetin"),
        inchikey: Some(arc("ABCDEF-GHIJKL-M")),
        smiles: Some(arc("O=c1cc(-c2ccccc2)oc2cc(O)cc(O)c12")),
        mass: Some(302.24),
        formula: Some(arc("C15H10O7")),
        taxon_qid: arc("Q128267"),
        taxon_name: arc("Rosa"),
        reference_qid: arc("Q100000001"),
        ref_title: Some(arc("Flavonoid isolation, 1971")),
        ref_doi: Some(arc("10.1000/a, b")),
        pub_year: Some(1971),
        statement: Some(arc("Q200000002")),
    }
}

/// A second row for the same compound, so interning and the "first write wins"
/// rule in [`SparseStrings`] are exercised rather than trivially satisfied.
fn second_occurrence() -> CompoundEntry {
    CompoundEntry {
        compound_qid: arc("Q3613679"),
        taxon_qid: arc("Q128267"),
        taxon_name: arc("Rosa"),
        reference_qid: arc("Q100000002"),
        ref_title: Some(arc("A \"quoted\" title")),
        ref_doi: Some(arc("10.1000/c")),
        pub_year: Some(1972),
        statement: Some(arc("Q200000003")),
        ..full_row()
    }
}

/// A different compound, so the set has more than one of everything.
fn other_row() -> CompoundEntry {
    CompoundEntry {
        compound_qid: arc("Q7"),
        name: arc("water"),
        inchikey: Some(arc("XLYOFNOQVPJJNP-UHFFFAOYSA-N")),
        smiles: Some(arc("O")),
        mass: Some(18.015),
        formula: Some(arc("H2O")),
        taxon_qid: arc("Q16521"),
        taxon_name: arc("Gentianales"),
        reference_qid: arc("Q100000003"),
        ref_title: None,
        ref_doi: None,
        pub_year: None,
        statement: Some(arc("Q200000004")),
    }
}

/// A row with everything optional absent, which is a different CSV shape from a
/// full one -- short cells, empty strings, an absent year.
fn sparse_row() -> CompoundEntry {
    CompoundEntry {
        compound_qid: arc("Q42"),
        name: arc(""),
        inchikey: None,
        smiles: None,
        mass: None,
        formula: None,
        taxon_qid: arc(""),
        taxon_name: arc(""),
        reference_qid: arc(""),
        ref_title: None,
        ref_doi: None,
        pub_year: None,
        statement: None,
    }
}

fn rows() -> Vec<CompoundEntry> {
    vec![full_row(), second_occurrence(), other_row(), sparse_row()]
}

/// Render the set the way the app does, through the real exporter.
fn render(format: ExportFormat, set: &ColumnarResultSet) -> String {
    let mut out = String::new();
    RowExporter::new(format, set).for_each_chunk(|chunk| out.push_str(chunk));
    out
}

/// A CSV in the dialect the *query* speaks, so the streaming reader sees the
/// header it will really be given.
///
/// The exporter speaks a different dialect -- it emits `compound_smiles`,
/// `ref` and `ref_year`, while the query projects `compound_smiles_iso`,
/// `compound_smiles_conn`, `ref_qid` and `ref_date`. That difference is real and
/// deliberate, and it is why this cannot simply be `render(ExportFormat::Csv, ..)`:
/// a round trip through the exporter would test the exporter against itself.
fn query_dialect_csv(set: &ColumnarResultSet) -> String {
    let mut out = String::new();
    out.push_str(&SELECT_COLUMNS.join(","));
    out.push('\n');
    for row in 0..set.row_count() {
        let entry = set.entry(row).expect("a row that was just built");
        let smiles = entry.smiles.as_deref().unwrap_or_default();
        // The query projects both SMILES columns; the parser prefers the isomeric
        // one, so that is where a value goes and the other stays empty. Sending
        // it to both would be a fixture no endpoint produces, since the store
        // keeps only one.
        let cells = [
            entry.compound_qid.to_string(),
            entry.name.to_string(),
            entry.inchikey.as_deref().unwrap_or_default().to_string(),
            String::new(),
            smiles.to_string(),
            entry.mass.map_or_else(String::new, |mass| mass.to_string()),
            entry.formula.as_deref().unwrap_or_default().to_string(),
            entry.taxon_qid.to_string(),
            entry.taxon_name.to_string(),
            entry.reference_qid.to_string(),
            // `?ref` is projected beside `?ref_qid` and the parser reads only the
            // latter, so the URI form is what a real response carries here.
            String::new(),
            entry.ref_title.as_deref().unwrap_or_default().to_string(),
            entry.ref_doi.as_deref().unwrap_or_default().to_string(),
            entry.pub_year.map_or_else(String::new, |y| y.to_string()),
            entry.statement.as_deref().unwrap_or_default().to_string(),
        ];
        for (index, cell) in cells.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str(&quote(cell));
        }
        out.push('\n');
    }
    out
}

/// RFC 4180 quoting, so a title with a comma or a quote survives the trip.
fn quote(cell: &str) -> String {
    if cell.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_string()
    }
}

/// Stream `payload` through the reader in slices of `chunk` bytes.
fn streamed(payload: &str, chunk: usize) -> ColumnarResultSet {
    let mut reader = CsvColumnarReader::new();
    let bytes = payload.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let end = (at + chunk).min(bytes.len());
        reader
            .feed(
                bytes
                    .get(at..end)
                    .expect("both ends are within the payload"),
            )
            .expect("a well-formed payload");
        at = end;
    }
    reader.finish().expect("a complete payload")
}

#[test]
fn every_export_format_is_identical_whichever_way_the_set_was_built() {
    let from_entries = ColumnarResultSet::from_entries(&rows());
    let from_csv = streamed(&query_dialect_csv(&from_entries), 4096);

    for format in [ExportFormat::Csv, ExportFormat::Json, ExportFormat::Rdf] {
        assert_eq!(
            render(format, &from_csv),
            render(format, &from_entries),
            "{format:?} differs between `from_entries` and the streaming reader. \
             One of them is wrong and the other is what the table shows, and this \
             test cannot say which -- that is what the golden files would be for."
        );
    }
}

#[test]
fn the_two_paths_agree_on_every_row_and_on_the_counts() {
    let from_entries = ColumnarResultSet::from_entries(&rows());
    let from_csv = streamed(&query_dialect_csv(&from_entries), 4096);

    assert_eq!(from_csv.row_count(), from_entries.row_count());
    assert_eq!(from_csv.stats(), from_entries.stats());
    for row in 0..from_entries.row_count() {
        assert_eq!(
            from_csv.entry(row),
            from_entries.entry(row),
            "row {row} differs between the two paths"
        );
    }
}

/// Chunk boundaries must not change the answer either. A reader that is correct
/// only when the transport happens to align is not correct, and this is the same
/// property `streaming.rs` asserts at splitter level -- here it is asserted on the
/// bytes that come out the other end.
#[test]
fn the_export_is_identical_at_every_chunk_size() {
    let from_entries = ColumnarResultSet::from_entries(&rows());
    let payload = query_dialect_csv(&from_entries);
    let expected = render(ExportFormat::Csv, &from_entries);

    // Every size, not a sample: a boundary inside a quoted field is the case that
    // breaks, and which field that is depends on where the cuts land.
    for chunk in 1..=payload.len() {
        let from_csv = streamed(&payload, chunk);
        assert_eq!(
            render(ExportFormat::Csv, &from_csv),
            expected,
            "a {chunk}-byte chunk boundary changed the exported CSV"
        );
    }
}

/// The truncation check, end to end through the reader.
///
/// `streaming.rs` covers `end_of_input` at splitter level, and `INCOMPLETE_BODY`
/// was reachable only from `finish()` with no test calling it that way. It is the
/// path a cut-off response takes, and it is the one the caller turns into a
/// refusal to show a partial result -- so a change to the reader that stopped
/// producing it would turn a truncated answer into a complete-looking one.
#[test]
fn a_body_cut_inside_a_quoted_field_is_refused_rather_than_parsed() {
    let mut reader = CsvColumnarReader::new();
    reader
        .feed(
            b"compound,compoundLabel,taxon,taxon_name,ref_qid,ref_title,statement\n\
                Q1,quercetin,Q2,Rosa,Q3,\"a title that never",
        )
        .expect("a payload cut inside a quoted field is not a feed error");
    let error = reader.finish().expect_err("a cut body must not parse");

    assert!(
        error.to_string().contains("incomplete"),
        "the error has to say the result set is incomplete, because that is what \
         the caller shows the reader instead of the rows: {error}"
    );
}

#[test]
fn a_body_ending_on_a_complete_row_is_not_truncation() {
    // The other side of the same boundary, and the reason the check is a check
    // rather than a rule that rejects every final row. No trailing newline, which
    // is what a body cut exactly at a record boundary looks like.
    let mut reader = CsvColumnarReader::new();
    reader
        .feed(
            b"compound,compoundLabel,taxon,taxon_name,ref_qid,statement\n\
                Q1,quercetin,Q2,Rosa,Q3,Q4",
        )
        .expect("a payload ending on a whole record is not a feed error");
    let set = reader.finish().expect("a complete final row must parse");

    assert_eq!(set.row_count(), 1);
}
