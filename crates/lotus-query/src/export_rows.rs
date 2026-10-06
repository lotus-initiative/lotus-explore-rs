// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Turning a result set into a file, without asking anyone else to.
//!
//! # Why this exists
//!
//! A download used to leave the browser, asking `QLever` or the API to re-run the query and
//! return a URL, which broke twice. The filename: the app names the file from the query and
//! shows that name on click, but the producing service decides it too, so `QLever` appends
//! its own suffix and the reader gets `lotus-export-1234.csv` after being told
//! `rosa-2026-10-03.csv`, with no way to reconcile since the app is out of the request by
//! then. And the query: the rows were already in memory, so the export made a second
//! endpoint produce again the answer sitting in the tab -- on a three-million-row search, a
//! second full execution of an expensive query.
//!
//! Both are fixed by not leaving: rows are read out of the set the table already holds, so
//! the shown filename is the only name in play.
//!
//! # Streaming
//!
//! A set is read out in chunks, not built whole. The set is already near the tab's budget and
//! a `String` of CSV would be a second copy: roughly 600 MB for two million rows, the failure
//! the streaming WDQS path avoids. [`RowExporter::next_chunk`] returns at most one chunk's
//! worth, and the caller hands each chunk on and drops it before asking for the next.
//!
//! The chunk boundary is deliberately not row-aligned: padding to the target size would mean
//! holding a partial row across calls, and a row is at most a few hundred bytes -- far less
//! than the slack that costs.

use crate::export::ExportFormat;
use lotus_model::{ColumnarResultSet, WIKIDATA_REFERENCE_BASE, WIKIDATA_STATEMENT_BASE};
use std::borrow::Cow;
use std::fmt::Write as _;

/// Target bytes per chunk.
///
/// Small on purpose: the chunk is the *only* copy of the export in memory at any moment, so
/// this number is the whole transient cost of a download, doubled briefly while copied into
/// JavaScript-owned storage.
///
/// 256 KiB was fine on a desktop and wrong on an iPhone. Per-chunk overhead is a pointer swap
/// and an await, so 256 KiB against 64 KiB is a few thousand extra awaits on a 600 MB export
/// -- nothing -- while the transient peak drops fourfold. Memory is scarce here and latency
/// is not.
const CHUNK_TARGET: usize = 64 * 1024;

/// The columns a locally-built export carries, in order.
///
/// The SPARQL variable names the endpoint uses, so a file from here and one from `QLever`
/// read as the same thing. Two of the endpoint's fifteen are absent because the set does not
/// keep them, and saying so beats emitting empty cells:
///
/// - `compound_smiles_iso` is not stored; only one SMILES per compound is, so which one
///   would be a guess.
/// - `ref` is the reference node as a URI; `ref` here carries the bare QID the set holds.
///
/// One is renamed: the set keeps only the publication *year*, so the column is `ref_year`.
/// Emitting `ref_date` with a year in it would be wrong rather than missing.
///
/// Deliberately *not* the same list as `SELECT_COLUMNS`: the reference node is projected for
/// the provenance graph and kept in the parsed row, but no column a person reads shows it.
/// Asserted by rendering a set and looking for the node's hash in the output, since a test
/// comparing the constants would only notice a rename.
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
    /// `Cow` because four of the thirteen are rendered rather than stored: the QIDs come out of
    /// a numeric dictionary and the statement out of packed bytes, so they need a buffer
    /// before they can be quoted. Borrowing keeps the other nine off the heap.
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

        // The occurrence is emitted as the reified statement chain the endpoint's
        // CONSTRUCT produces, so a consumer that parses one parses the other:
        //
        //     compound p:P703  statement
        //     statement ps:P703              taxon
        //     statement prov:wasDerivedFrom reference-node
        //     reference-node pr:P248         publication
        //
        // Attaching taxon and publication to the compound directly, with the compound as
        // the subject of `prov:wasDerivedFrom`, is a different graph, not a shorter
        // spelling: the *statement* is what was derived from the reference, and two
        // occurrences of one compound through different references became
        // indistinguishable. It also dropped the reference node, which is why `?ref` has
        // to be in the projection -- without it there is nothing to point at.
        let taxon = set
            .taxon_qid_text(row)
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        let statement = statement_text(set, row);
        // Bracketed: a bare URI is not a Turtle IRI, and the endpoint's own export
        // emits them bracketed. Unbracketed, a strict parser rejects the file.
        let statement = (!statement.trim().is_empty())
            .then(|| format!("<{WIKIDATA_STATEMENT_BASE}{}>", statement.trim()));

        match (&statement, &taxon) {
            (Some(statement_uri), Some(taxon)) => {
                self.emit(&subject, "p:P703", statement_uri);
                self.emit(statement_uri, "ps:P703", &format!("wd:{taxon}"));
            }
            // No statement to hang it on: fall back to the direct taxon edge, which
            // is lossy but keeps the occurrence in the graph.
            (None, Some(taxon)) => self.emit(&subject, "wdt:P703", &format!("wd:{taxon}")),
            _ => {}
        }

        if let Some(taxon) = &taxon
            && let Some(label) = set.taxon_label(row)
        {
            self.emit(&format!("wd:{taxon}"), "wdt:P225", &quote(Some(label)));
        }

        // `prov:wasDerivedFrom` names the reference *node*, and `pr:P248` links
        // that node to the publication. They are different identifiers and the
        // graph needs both.
        let reference_node = set
            .reference_node_text(row)
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            // The stored value is the node's hash with the namespace already
            // stripped, so the namespace goes back on here. An absolute URI is
            // left alone: not every `prov:wasDerivedFrom` object is a
            // `/reference/<hex>` node, and prepending the namespace to one of those
            // produced `.../reference/http://www.wikidata.org/r`.
            .map(|hash| {
                if hash.starts_with("http://") || hash.starts_with("https://") {
                    format!("<{hash}>")
                } else {
                    format!("<{WIKIDATA_REFERENCE_BASE}{hash}>")
                }
            });
        let publication = set
            .reference_qid_text(row)
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty())
            .map(|qid| format!("wd:{qid}"));

        let reference_term = reference_node.as_deref().or(publication.as_deref());

        if let Some(statement_uri) = &statement
            && let Some(reference) = reference_term
        {
            self.emit(statement_uri, "prov:wasDerivedFrom", reference);
        }
        if let (Some(node), Some(publication)) = (&reference_node, &publication) {
            self.emit(node, "pr:P248", publication);
        }

        // Title, DOI and year live on the publication, not on the reference node.
        if let Some(publication) = &publication {
            self.emit(publication, "wdt:P1476", &quote(set.reference_title(row)));
            self.emit(publication, "wdt:P356", &quote(set.reference_doi(row)));
            if let Some(year) = set.pub_year(row) {
                self.emit(publication, "wdt:P577", &format!("\"{year}\"^^xsd:gYear"));
            }
        }
    }

    fn emit(&mut self, subject: &str, predicate: &str, object: &str) {
        // An empty object means the value was absent, and a triple whose object is
        // unbound is not emitted -- which is what the endpoint's CONSTRUCT does, and
        // what makes this file readable by the same tooling.
        if object.is_empty() || subject.is_empty() {
            return;
        }
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
@prefix p: <http://www.wikidata.org/prop/> .\n\
@prefix ps: <http://www.wikidata.org/prop/statement/> .\n\
@prefix pr: <http://www.wikidata.org/prop/reference/> .\n\
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
/// A Turtle typed number, or nothing when there is none.
///
/// Empty rather than `[]`, for the same reason `quote` is: an empty blank node is a node,
/// and `wdt:P2067 []` says a compound has a molecular mass of nothing. `emit` drops the
/// triple, as the endpoint's CONSTRUCT does with an unbound object.
fn number(value: Option<f64>) -> String {
    value
        .filter(|mass| mass.is_finite())
        .map_or_else(String::new, |mass| {
            let text = if mass.fract() == 0.0 {
                format!("{mass:.1}")
            } else {
                format!("{mass}")
            };
            format!("\"{text}\"^^xsd:decimal")
        })
}

/// A Turtle object: a quoted literal, or nothing for a missing value.
fn quote(value: Option<&str>) -> String {
    // An absent value emits no triple. Emitting `[]` instead would be valid syntax and a
    // node meaning nothing, leaving `wdt:P235 []` for every compound missing an InChIKey,
    // which a consumer cannot tell from a real value. This is the `CONSTRUCT` semantics the
    // endpoint follows too: a triple with an unbound object is not produced.
    let Some(value) = value.map(str::trim).filter(|v| !v.is_empty()) else {
        return String::new();
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
#[path = "export_rows/tests.rs"]
mod tests;
