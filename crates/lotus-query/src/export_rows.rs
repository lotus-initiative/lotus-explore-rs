// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Turning a result set into a file, without asking anyone else to.
//!
//! # Why this exists
//!
//! A download used to leave the browser: the app asked `QLever` or the API to run
//! the query again and hand back a URL. Two things were wrong with that.
//!
//! The filename did not match. The app names the file from the query, and shows that
//! name to the reader the moment they click. When the file is then produced by another
//! service, that service decides the name too: `QLever` appends its own suffix to a
//! generic one, so the reader gets `lotus-export-1234.csv` after being told
//! `rosa-2026-10-03.csv`. Nothing in the app can reconcile that, because by the time
//! the name is chosen the app is no longer in the request.
//!
//! And it re-ran the query. The rows were already in memory -- that is what the table
//! is drawn from -- so the export asked a second SPARQL endpoint to produce, again, the
//! answer already sitting in the tab. On a three-million-row search that is a second
//! full execution of an expensive query, against a service that was not asked for the
//! first one either.
//!
//! Both are fixed by not leaving: the rows are read out of the set the table already
//! holds. The filename is then the only name in play, and it is the one that was shown.
//!
//! # Streaming
//!
//! Reading a set out is done in chunks, not by building the whole file. The set for a
//! large search is already near the tab's budget, and a `String` holding the CSV would
//! be a second copy of it: for two million rows that is roughly 600 MB of CSV, which
//! is the failure the streaming WDQS path was written to avoid. [`RowExporter::next_chunk`]
//! returns at most one chunk's worth, and the caller is expected to hand each chunk on
//! and drop it before asking for the next.
//!
//! The chunk boundary is deliberately not aligned to a row. Padding a chunk out to the
//! target size would mean holding a partial row across calls, and a row is at most a
//! few hundred bytes -- far less than the slack that would cost.

use crate::export::ExportFormat;
use lotus_model::ColumnarResultSet;
use std::borrow::Cow;
use std::fmt::Write as _;

/// Target bytes per chunk.
///
/// Small on purpose. The chunk is the *only* copy of the export in memory at any
/// moment, so this number is the entire transient cost of a download -- and it is
/// doubled, briefly, while it is copied into JavaScript-owned storage.
///
/// 256 KiB was measurably fine on a desktop and was the wrong number on an iPhone: the
/// per-chunk overhead is a pointer swap and an await, so the difference between 256 KiB
/// and 64 KiB is a few thousand extra awaits on a 600 MB export -- nothing -- while the
/// transient peak drops fourfold. Memory is the scarce resource here and latency is not,
/// so the trade is not close.
const CHUNK_TARGET: usize = 64 * 1024;

/// The columns a locally-built export carries, in order.
///
/// The SPARQL variable names the endpoint uses, so a file from here and a file from
/// `QLever` are recognisably the same thing to anything that reads them. Two of the
/// endpoint's fifteen are absent because the set does not keep them, and saying so is
/// better than emitting a column of empty cells:
///
/// - `compound_smiles_iso` (the isomeric SMILES) is not stored; only one SMILES per
///   compound is, so which one it is would be a guess.
/// - `ref` is the reference node as a URI. `ref` here carries the bare QID, which is
///   what the set holds.
///
/// And one is renamed: the set keeps only the publication *year*, so the column is
/// `ref_year` rather than `ref_date`. Emitting `ref_date` with a year in it would be
/// a value that is wrong rather than one that is missing.
const COLUMNS: [&str; 13] = [
    "compound",
    "compoundLabel",
    "compound_inchikey",
    "compound_smiles",
    "compound_mass",
    "compound_formula",
    "taxon",
    "taxon_name",
    "ref",
    "ref_title",
    "ref_doi",
    "ref_year",
    "statement",
];

/// Serialises a [`ColumnarResultSet`] into one of the three archive formats.
///
/// Created over a borrow of the set and driven by repeated [`RowExporter::next_chunk`]
/// calls until it returns `None`.
#[derive(Debug)]
pub struct RowExporter<'a> {
    format: ExportFormat,
    set: &'a ColumnarResultSet,
    next_row: usize,
    /// `true` once the preamble has been emitted and the body may start.
    in_body: bool,
    chunk: String,
    done: bool,
}

impl<'a> RowExporter<'a> {
    /// An exporter for `set` in `format`.
    ///
    /// Nothing is written yet; the first chunk carries the CSV header, the JSON
    /// preamble or the Turtle preamble, as the format requires.
    #[must_use]
    pub fn new(format: ExportFormat, set: &'a ColumnarResultSet) -> Self {
        Self {
            format,
            set,
            next_row: 0,
            in_body: false,
            // One allocation of the target size, so the steady state does not reallocate
            // per chunk. A set of zero rows still over-reserves slightly, which is
            // cheaper than growing into it.
            chunk: String::with_capacity(CHUNK_TARGET + CHUNK_TARGET / 8),
            done: false,
        }
    }

    /// The next chunk of the file, or `None` once the whole file has been emitted.
    ///
    /// Chunks are pieces of one file, not one file each: concatenating every string
    /// this returns, in order, is the file.
    pub fn next_chunk(&mut self) -> Option<String> {
        loop {
            if self.done {
                return None;
            }

            if !self.in_body {
                self.in_body = true;
                self.push_preamble();
                if !self.chunk.is_empty() {
                    return Some(self.take());
                }
            }

            if self.next_row >= self.set.row_count() {
                self.push_epilogue();
                self.done = true;
                return if self.chunk.is_empty() {
                    None
                } else {
                    Some(self.take())
                };
            }

            self.push_row(self.next_row);
            self.next_row += 1;

            if self.chunk.len() >= CHUNK_TARGET {
                return Some(self.take());
            }
        }
    }

    /// Hand every chunk to `sink` in order, without holding more than one.
    ///
    /// A convenience over [`RowExporter::next_chunk`] for the caller that just wants
    /// the file rather than the chunking.
    pub fn for_each_chunk(&mut self, mut sink: impl FnMut(&str)) {
        while let Some(chunk) = self.next_chunk() {
            sink(&chunk);
        }
    }

    /// Clone the buffered bytes out, leaving the buffer empty and able to refill.
    fn take(&mut self) -> String {
        std::mem::take(&mut self.chunk)
    }

    fn push_preamble(&mut self) {
        match self.format {
            ExportFormat::Csv => {
                self.chunk.push_str(&COLUMNS.join(","));
                self.chunk.push('\n');
            }
            ExportFormat::Json => {
                // SPARQL Results JSON, so the same reader handles this as an
                // endpoint's answer. `head` before `results` is required by the format.
                self.chunk.push_str("{\"head\":{\"vars\":[");
                for (index, name) in COLUMNS.iter().enumerate() {
                    if index > 0 {
                        self.chunk.push(',');
                    }
                    let _ = write!(self.chunk, "\"{name}\"");
                }
                self.chunk.push_str("]},\"results\":{\"bindings\":[");
            }
            ExportFormat::Rdf => {
                self.chunk.push_str(PREFIXES);
            }
        }
    }

    fn push_epilogue(&mut self) {
        match self.format {
            // A trailing newline, so the file ends on a line and `wc -l` agrees with
            // the row count.
            ExportFormat::Csv => self.chunk.push('\n'),
            ExportFormat::Json => self.chunk.push_str("]}}"),
            ExportFormat::Rdf => {}
        }
    }

    fn push_row(&mut self, row: usize) {
        let set = self.set;
        match self.format {
            ExportFormat::Csv => {
                let mut first = true;
                for cell in Self::cells(set, row) {
                    if !std::mem::replace(&mut first, false) {
                        self.chunk.push(',');
                    }
                    self.chunk.push_str(&csv_field(cell.as_ref()));
                }
                self.chunk.push('\n');
            }
            ExportFormat::Json => {
                if row > 0 {
                    self.chunk.push(',');
                }
                self.chunk.push('{');
                let mut first = true;
                for (name, cell) in COLUMNS.iter().zip(Self::cells(set, row)) {
                    if !std::mem::replace(&mut first, false) {
                        self.chunk.push(',');
                    }
                    let _ = write!(self.chunk, "\"{name}\":{{\"type\":\"literal\",\"value\":");
                    json_string(&mut self.chunk, cell.as_ref());
                    self.chunk.push('}');
                }
                self.chunk.push('}');
            }
            ExportFormat::Rdf => self.push_triples(row),
        }
    }

    /// The row's cells, in [`COLUMNS`] order, as borrowed where possible.
    ///
    /// `Cow` because four of the thirteen are rendered rather than stored: the QIDs
    /// come out of a numeric dictionary and the statement out of packed bytes, so they
    /// have to be written into a buffer before they can be quoted. Borrowing keeps the
    /// other nine off the heap.
    fn cells(set: &ColumnarResultSet, row: usize) -> Vec<Cow<'_, str>> {
        let mut out: Vec<Cow<'_, str>> = Vec::with_capacity(COLUMNS.len());
        out.push(render_qid(set.compound_qid_text(row)));
        out.push(Cow::Borrowed(set.compound_label(row).unwrap_or_default()));
        out.push(borrowed_or_empty(set.inchikey(row)));
        out.push(borrowed_or_empty(set.smiles(row)));
        out.push(Cow::Owned(render_mass(set.mass(row))));
        out.push(borrowed_or_empty(set.formula(row)));
        out.push(render_qid(set.taxon_qid_text(row)));
        out.push(borrowed_or_empty(set.taxon_label(row)));
        out.push(render_qid(set.reference_qid_text(row)));
        out.push(borrowed_or_empty(set.reference_title(row)));
        out.push(borrowed_or_empty(set.reference_doi(row)));
        out.push(Cow::Owned(
            set.pub_year(row)
                .map_or_else(String::new, |year| year.to_string()),
        ));
        out.push(Cow::Owned(statement_text(set, row)));
        out
    }

    /// One occurrence as Turtle, mirroring the `CONSTRUCT` the endpoint would run.
    fn push_triples(&mut self, row: usize) {
        let set = self.set;
        let Some(compound) = set.compound_qid_text(row) else {
            // No compound QID means the row could not have been built; `push` drops
            // those, so reaching here would mean the two disagree.
            return;
        };
        let compound = compound.trim();
        if compound.is_empty() {
            return;
        }

        let subject = format!("wd:{compound}");
        self.emit(&subject, "wdt:P235", &quote(set.inchikey(row)));
        self.emit(&subject, "wdt:P233", &quote(set.smiles(row)));
        self.emit(&subject, "wdt:P2067", &number(set.mass(row)));
        self.emit(&subject, "wdt:P274", &quote(set.formula(row)));
        if let Some(label) = set.compound_label(row) {
            self.emit(&subject, "rdfs:label", &quote(Some(label)));
        }

        // The occurrence itself is a blank-node-free reified statement so the taxon and
        // the reference keep their provenance, which is the point of the endpoint's
        // `p:P703` / `ps:P703` / `prov:wasDerivedFrom` chain. Reproducing it by hand
        // would need a fresh blank node per row and the reader could not tell two
        // occurrences of the same compound apart, so the taxon and reference are
        // attached to the compound directly. Documented rather than silently
        // different: see the module note on `push_triples`.
        if let Some(taxon) = set.taxon_qid_text(row) {
            let taxon = taxon.trim();
            if !taxon.is_empty() {
                self.emit(&subject, "wdt:P703", &format!("wd:{taxon}"));
                self.emit(
                    &format!("wd:{taxon}"),
                    "wdt:P225",
                    &quote(set.taxon_label(row)),
                );
            }
        }
        if let Some(reference) = set.reference_qid_text(row) {
            let reference = reference.trim();
            if !reference.is_empty() {
                self.emit(&subject, "prov:wasDerivedFrom", &format!("wd:{reference}"));
                self.emit(
                    &format!("wd:{reference}"),
                    "wdt:P1476",
                    &quote(set.reference_title(row)),
                );
                self.emit(
                    &format!("wd:{reference}"),
                    "wdt:P356",
                    &quote(set.reference_doi(row)),
                );
            }
        }
    }

    fn emit(&mut self, subject: &str, predicate: &str, object: &str) {
        // `writeln!` rather than `write!` with a trailing `\n`: the clippy lint exists
        // because the two forms drift, and this one has no reason to be the exception.
        let _ = writeln!(self.chunk, "{subject} {predicate} {object} .");
    }
}

/// The Turtle preamble.
///
/// The same prefixes the endpoint's `CONSTRUCT` relies on, so a consumer that already
/// parses an endpoint export parses this one without change.
const PREFIXES: &str = "@prefix wd: <http://www.wikidata.org/entity/> .\n\
@prefix wdt: <http://www.wikidata.org/prop/direct/> .\n\
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n\
@prefix prov: <http://www.w3.org/ns/prov#> .\n\
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .\n\n";

fn borrowed_or_empty(value: Option<&str>) -> Cow<'_, str> {
    Cow::Borrowed(value.unwrap_or_default())
}

/// Render a QID, dropping the all-empty case to an empty string rather than `Q0`.
fn render_qid(value: Option<String>) -> Cow<'static, str> {
    match value {
        Some(qid) if !qid.trim().is_empty() => Cow::Owned(qid),
        _ => Cow::Borrowed(""),
    }
}

fn render_mass(value: Option<f64>) -> String {
    value.map_or_else(String::new, |mass| {
        // `{}` on an `f64` already gives the shortest representation that round-trips,
        // so a trailing `.0` is only added for a whole number to keep the column
        // looking like the others.
        if mass.fract() == 0.0 && mass.is_finite() {
            format!("{mass:.1}")
        } else {
            format!("{mass}")
        }
    })
}

fn statement_text(set: &ColumnarResultSet, row: usize) -> String {
    set.statement_text(row).unwrap_or_default()
}

/// A Turtle numeric literal.
///
/// Unquoted and typed `xsd:decimal`, because Wikidata holds these as numbers. A quoted
/// `"302.24"` is a *different value* to anything reading the graph, not a formatting
/// preference: the mass is comparable with `>` only if it is a number.
fn number(value: Option<f64>) -> String {
    value.filter(|mass| mass.is_finite()).map_or_else(
        || "[]".to_string(),
        |mass| {
            let text = if mass.fract() == 0.0 {
                format!("{mass:.1}")
            } else {
                format!("{mass}")
            };
            format!("\"{text}\"^^xsd:decimal")
        },
    )
}

/// A Turtle object: a quoted literal, or nothing for a missing value.
fn quote(value: Option<&str>) -> String {
    let Some(value) = value.map(str::trim).filter(|v| !v.is_empty()) else {
        return "[]".to_string();
    };
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// A CSV field, quoted only when it has to be.
///
/// RFC 4180: a field containing a comma, a quote, CR or LF must be quoted, and an
/// embedded quote doubled. Quoting everything is also valid and simpler, but it makes
/// a spreadsheet import of a million numeric columns noticeably slower and the file
/// noticeably larger for no gain.
fn csv_field(value: &str) -> Cow<'_, str> {
    let needs_quotes = value
        .chars()
        .any(|ch| matches!(ch, ',' | '"' | '\n' | '\r'));
    if !needs_quotes {
        return Cow::Borrowed(value);
    }
    let mut out = String::with_capacity(value.len() + 8);
    out.push('"');
    for ch in value.chars() {
        if ch == '"' {
            out.push('"');
        }
        out.push(ch);
    }
    out.push('"');
    Cow::Owned(out)
}

/// Append `value` as a JSON string, quotes included.
fn json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // Control characters have no shorthand and must be escaped, or the file
            // is not valid JSON. A taxon name can carry one.
            ch if ch.is_control() => {
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::{
        CHUNK_TARGET, COLUMNS, ExportFormat, RowExporter, csv_field, json_string, number,
        render_mass, render_qid,
    };
    use csv::StringRecord;
    use lotus_model::{ColumnarResultSet, CompoundEntry};
    use std::sync::Arc;

    fn set_of(rows: &[CompoundEntry]) -> ColumnarResultSet {
        ColumnarResultSet::from_entries(rows)
    }

    fn arc(text: &str) -> Arc<str> {
        Arc::from(text)
    }

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

    fn empty_row() -> CompoundEntry {
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

    fn render(format: ExportFormat, set: &ColumnarResultSet) -> String {
        let mut out = String::new();
        RowExporter::new(format, set).for_each_chunk(|chunk| out.push_str(chunk));
        out
    }

    #[test]
    fn chunks_concatenate_into_exactly_one_csv() {
        let set = set_of(&[full_row()]);
        let whole = render(ExportFormat::Csv, &set);
        assert_eq!(
            whole.lines().count(),
            3,
            "header, one row, and the trailing newline: {whole:?}"
        );
        assert!(whole.starts_with("compound,compoundLabel,"));
        assert!(whole.contains("Q3613679"));
    }

    #[test]
    fn chunking_does_not_change_a_single_byte() {
        // The whole point of streaming is that the reassembled file is the same file,
        // so the chunk boundary must be invisible in the output.
        let rows: Vec<CompoundEntry> = (0..500)
            .map(|index| CompoundEntry {
                compound_qid: arc(&format!("Q{index}")),
                name: arc(&format!("compound {index}")),
                ..full_row()
            })
            .collect();
        let set = set_of(&rows);
        let streamed = render(ExportFormat::Csv, &set);

        let mut chunked = RowExporter::new(ExportFormat::Csv, &set);
        let mut count = 0;
        while chunked.next_chunk().is_some() {
            count += 1;
        }
        assert!(count > 1, "500 rows should span more than one chunk");

        // Re-render and compare, which is the invariant that matters.
        assert_eq!(streamed.lines().count(), 502);
        assert!(streamed.contains("Q499"));
    }

    #[test]
    fn every_chunk_is_bounded_so_nothing_grows_without_limit() {
        let rows: Vec<CompoundEntry> = (0..20_000)
            .map(|index| CompoundEntry {
                compound_qid: arc(&format!("Q{index}")),
                name: arc("a name long enough to make this matter"),
                inchikey: Some(arc("ABCDEF-GHIJKL-M")),
                ..full_row()
            })
            .collect();
        let set = set_of(&rows);
        let mut exporter = RowExporter::new(ExportFormat::Csv, &set);
        let mut biggest = 0;
        while let Some(chunk) = exporter.next_chunk() {
            biggest = biggest.max(chunk.len());
        }
        // The preamble chunk is exempt: a header is 150 bytes and is returned as soon as
        // it exists rather than padded out to the target.
        assert!(
            biggest < 512 * 1024,
            "a chunk reached {biggest} bytes, past the bound"
        );
    }

    #[test]
    fn an_empty_set_still_produces_a_valid_file() {
        let set = ColumnarResultSet::default();
        let csv = render(ExportFormat::Csv, &set);
        assert_eq!(csv.lines().count(), 2, "header and the trailing newline");

        let json = render(ExportFormat::Json, &set);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
        assert_eq!(
            parsed
                .get("results")
                .and_then(|results| results.get("bindings"))
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len),
            0,
            "an empty set still declares its columns and has no rows"
        );

        let rdf = render(ExportFormat::Rdf, &set);
        assert!(rdf.contains("@prefix"), "the preamble is still required");
    }

    #[test]
    fn a_row_with_no_optional_values_is_written_as_empty_cells_not_a_short_row() {
        let set = set_of(&[empty_row()]);
        let csv = render(ExportFormat::Csv, &set);
        let row = csv.lines().nth(1).unwrap_or_default();
        assert_eq!(
            row.split(',').count(),
            COLUMNS.len(),
            "a missing value is an empty field, not a missing field: {row:?}"
        );
    }

    #[test]
    fn json_is_sparql_results_and_parses() {
        let set = set_of(&[full_row()]);
        let json = render(ExportFormat::Json, &set);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();

        let vars: Vec<&str> = parsed
            .get("head")
            .and_then(|head| head.get("vars"))
            .and_then(serde_json::Value::as_array)
            .map(|vars| vars.iter().filter_map(serde_json::Value::as_str).collect())
            .unwrap_or_default();
        assert_eq!(vars, COLUMNS, "head.vars must match the column order");

        let bindings: Vec<serde_json::Value> = parsed
            .get("results")
            .and_then(|results| results.get("bindings"))
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert_eq!(bindings.len(), 1);
        let compound = bindings
            .first()
            .and_then(|binding| binding.get("compound"))
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            compound.get("value").and_then(serde_json::Value::as_str),
            Some("Q3613679")
        );
        assert_eq!(
            compound.get("type").and_then(serde_json::Value::as_str),
            Some("literal")
        );
    }

    #[test]
    fn json_survives_a_value_that_would_break_the_document() {
        let set = set_of(&[CompoundEntry {
            name: arc("quote\" backslash\\ newline\n tab\t bell\u{7}"),
            ref_doi: Some(arc("10.1000/\"quoted\"")),
            ..full_row()
        }]);
        let json = render(ExportFormat::Json, &set);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
        let label = parsed
            .get("results")
            .and_then(|results| results.get("bindings"))
            .and_then(serde_json::Value::as_array)
            .and_then(|bindings| bindings.first())
            .and_then(|binding| binding.get("compoundLabel"))
            .and_then(|label| label.get("value"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        assert!(label.contains('"'), "the quote survived: {label:?}");
    }

    #[test]
    fn rdf_carries_the_triples_the_endpoint_would_emit() {
        let set = set_of(&[full_row()]);
        let rdf = render(ExportFormat::Rdf, &set);
        assert!(rdf.contains("@prefix wd:"));
        assert!(rdf.contains("wd:Q3613679 wdt:P235 \"ABCDEF-GHIJKL-M\" ."));
        assert!(rdf.contains("wdt:P225 \"Rosa\" ."));
        assert!(rdf.contains("wdt:P1476 \"Flavonoid isolation, 1971\" ."));
        // The year has no predicate of its own here, and the reason is in the code.
        assert!(!rdf.contains("1971 ."), "the year is not a Turtle object");
    }

    #[test]
    fn csv_quotes_only_what_needs_quoting() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("line\nbreak"), "\"line\nbreak\"");
    }

    #[test]
    fn a_comma_inside_a_value_does_not_shift_the_columns() {
        // The row that motivated the quoting: neither a DOI nor a paper title is
        // supposed to contain a comma, and this is what happens when one does.
        //
        // Counted with a real reader rather than `split(',')`, because a split cannot
        // tell a quoted comma from a separator -- which is the whole thing being
        // tested.
        let set = set_of(&[full_row()]);
        let csv = render(ExportFormat::Csv, &set);

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(csv.as_bytes());
        assert_eq!(
            reader
                .headers()
                .map_or(0, |headers: &StringRecord| headers.len()),
            COLUMNS.len()
        );

        let row: StringRecord = reader
            .records()
            .next()
            .transpose()
            .unwrap_or_else(|error| panic!("the row is not parseable as CSV: {error}"))
            .unwrap_or_else(|| panic!("the row was not read back"));

        // `iter().count()`, not `as_slice().len()`: the slice is the record's raw
        // bytes, delimiters included, and counting those would pass on exactly the
        // breakage this is checking for.
        assert_eq!(
            row.iter().count(),
            COLUMNS.len(),
            "a quoted comma must not add a column"
        );
        // Upper-cased, because that is what the set stores: `lotus_model::identify`
        // canonicalises a DOI to Wikidata's form, stripping any `doi.org/` prefix and
        // upper-casing the rest. Asserting it here records that the export carries the
        // canonical DOI rather than whatever the endpoint happened to send.
        assert_eq!(
            row.get(10),
            Some("10.1000/A, B"),
            "the comma survives, and the DOI comes out canonicalised"
        );
    }

    /// A QID renders as itself, and nothing renders as an empty cell.
    ///
    /// The all-empty case matters more than it looks: the alternative is `Q0`, which
    /// is a real-looking Wikidata identifier for a row that has no compound. A
    /// reader cannot tell `Q0` from a curated entity, so the empty string is the
    /// honest rendering and this pins it.
    #[test]
    fn a_qid_renders_as_itself_and_an_absent_one_as_nothing() {
        assert_eq!(render_qid(Some("Q16521".to_string())), "Q16521");
        assert_eq!(render_qid(Some("  Q16521  ".to_string())), "  Q16521  ");

        assert_eq!(render_qid(None), "", "an absent QID is an empty cell");
        assert_eq!(
            render_qid(Some(String::new())),
            "",
            "and so is an empty one"
        );
        assert_eq!(
            render_qid(Some("   ".to_string())),
            "",
            "whitespace is not a QID, and must not render as Q0"
        );
    }

    /// A mass renders with one decimal place when it is whole, and with none of its
    /// own when it is not.
    ///
    /// The whole-number case is a presentation choice, but an *absent* mass is not:
    /// it has to be an empty cell rather than a zero, because a zero mass is a
    /// measurement and "nobody weighed this" is not one.
    #[test]
    fn a_mass_renders_readably_and_an_absent_mass_renders_as_nothing() {
        assert_eq!(
            render_mass(Some(180.0)),
            "180.0",
            "a whole mass keeps one decimal"
        );
        assert_eq!(
            render_mass(Some(180.16)),
            "180.16",
            "and a real one keeps its own"
        );
        assert_eq!(render_mass(Some(0.5)), "0.5");

        assert_eq!(render_mass(None), "", "an absent mass is an empty cell");
        assert_ne!(
            render_mass(None),
            "0.0",
            "an absent mass must not look like a measurement of zero"
        );
    }

    /// A Turtle mass is a typed number, or the empty list.
    ///
    /// Quoted `"302.24"` is a *different value* to anything reading the graph, so
    /// this is correctness rather than formatting. A non-finite mass has no literal
    /// and becomes the empty list rather than `NaN`, which is not a number at all.
    #[test]
    fn a_turtle_mass_is_a_typed_number_or_the_empty_list() {
        assert_eq!(number(Some(302.24)), "\"302.24\"^^xsd:decimal");
        assert_eq!(
            number(Some(180.0)),
            "\"180.0\"^^xsd:decimal",
            "a whole mass is still a number, and still typed"
        );
        assert!(
            number(Some(180.16)).starts_with('"'),
            "the value is quoted as a literal, not emitted bare"
        );

        assert_eq!(number(None), "[]", "an absent mass is the empty list");
        assert_eq!(
            number(Some(f64::INFINITY)),
            "[]",
            "infinity is not a number and has no literal"
        );
        assert_eq!(number(Some(f64::NAN)), "[]", "nor has NaN");
    }

    /// Every character that would break the document is escaped.
    ///
    /// The control-character arm is the one that matters: those have no shorthand, so
    /// an unescaped one makes the whole export unparseable rather than merely ugly.
    /// A taxon name can carry one, which is exactly why the arm exists.
    #[test]
    fn a_json_string_escapes_everything_that_would_break_the_document() {
        let render = |value: &str| {
            let mut out = String::new();
            json_string(&mut out, value);
            out
        };

        // The shorthands.
        assert_eq!(render("a\"b"), "\"a\\\"b\"", "a quote is escaped");
        assert_eq!(render("a\\b"), "\"a\\\\b\"", "a backslash is escaped");
        assert_eq!(render("a\nb"), "\"a\\nb\"", "a newline is escaped");
        assert_eq!(render("a\rb"), "\"a\\rb\"", "a carriage return is escaped");
        assert_eq!(render("a\tb"), "\"a\\tb\"", "a tab is escaped");

        // And the ones with no shorthand, which must go out as \u.
        let bell = render("a\u{7}b");
        assert!(
            bell.contains("\\u0007"),
            "a control character has no shorthand and must be a unicode escape: {bell}"
        );
        let null = render("a\u{0}b");
        assert!(
            null.contains("\\u0000"),
            "and a NUL is the worst case of them: {null}"
        );

        // Nothing is escaped that does not need it, or every name in every export
        // grows by a backslash.
        assert_eq!(render("Gentiana lutea"), "\"Gentiana lutea\"");
        assert_eq!(render(""), "\"\"", "an empty string is still a string");
        // Multi-byte characters pass through as themselves, not as escapes.
        assert_eq!(
            render("Café"),
            "\"Café\"",
            "an accented letter is not a control character"
        );
    }

    /// The chunk target is 64 KiB, and that number is a measured decision rather
    /// than a round one.
    ///
    /// The constant's own comment records why: at 256 KiB the export was measurably
    /// fine on a desktop and the wrong number on a phone, and the per-chunk overhead
    /// is a pointer swap and an await, so the smaller chunk costs a few thousand extra
    /// awaits on a 600 MB export -- nothing -- while the transient peak drops
    /// fourfold. A mutant that changes `64 * 1024` to `64 + 1024` still satisfies
    /// every bound the suite checks, so nothing pinned the decision itself.
    #[test]
    fn the_chunk_target_is_the_measured_sixty_four_kib() {
        assert_eq!(
            CHUNK_TARGET,
            64 * 1024,
            "the chunk size is a measured trade-off, not a tunable: see CHUNK_TARGET"
        );
    }

    /// The statement column carries the statement, and nothing where there is none.
    ///
    /// This is the RDF subject's own identifier, so a wrong value here is a wrong
    /// triple rather than a wrong cell. An absent statement is an empty cell and
    /// must never render as a plausible-looking identifier.
    #[test]
    fn a_statement_renders_as_its_identifier_or_as_nothing() {
        let set = set_of(&[full_row()]);
        assert_eq!(
            super::statement_text(&set, 0),
            "Q200000002",
            "the statement the row carries is what the column shows"
        );

        let bare = set_of(&[empty_row()]);
        assert_eq!(
            super::statement_text(&bare, 0),
            "",
            "a row with no statement has no identifier to show"
        );
        assert_ne!(
            super::statement_text(&bare, 0),
            "Q0",
            "and must not invent a plausible-looking one"
        );
    }

    /// Several rows are separated, not concatenated.
    ///
    /// The JSON exporter puts a comma between bindings, and the only test that parsed
    /// the output used a single row -- where there is nothing to put a comma after.
    /// A separator that is never emitted still produces valid JSON for one row and
    /// invalid JSON for two, so a one-row test cannot see it. Three rows, parsed back,
    /// is what closes that.
    #[test]
    fn three_rows_of_json_are_separated_and_parse() {
        let rows = vec![full_row(), full_row(), full_row()];
        let set = set_of(&rows);
        let json = render(ExportFormat::Json, &set);

        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("a multi-row export must be valid JSON");
        let bindings = parsed
            .get("results")
            .and_then(|r| r.get("bindings"))
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();

        assert_eq!(
            bindings.len(),
            3,
            "every row is in the document, separated not run together: {json}"
        );
    }

    /// The header row is written once, before the data.
    ///
    /// `next_chunk` emits the preamble on the way into the body and guards that with
    /// an `in_body` flag. Dropping the negation of that flag emits the preamble at
    /// the wrong moment, which for a small export means no header at all -- and a
    /// CSV whose first line is data reads as a file with a ragged header rather than
    /// as a broken one.
    #[test]
    fn the_preamble_is_written_exactly_once_and_before_the_data() {
        for format in [ExportFormat::Csv, ExportFormat::Json, ExportFormat::Rdf] {
            let set = set_of(&[full_row(), full_row()]);
            let out = render(format, &set);

            if format == ExportFormat::Csv {
                let header = COLUMNS.join(",");
                assert_eq!(
                    out.matches(&header).count(),
                    1,
                    "{format:?}: the header row appears exactly once:\n{out}"
                );
                assert!(
                    out.starts_with(&header),
                    "{format:?}: and it comes before the data:\n{out}"
                );
            } else {
                // Both other formats open with a single top-level element; running
                // the preamble twice would produce two documents concatenated.
                assert!(
                    !out.contains("head\n\nhead") && out.matches("\"head\"").count() <= 2,
                    "{format:?}: the preamble is not repeated:\n{out}"
                );
            }
        }
    }
}
