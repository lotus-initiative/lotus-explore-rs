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

mod csv;
mod json;
mod turtle;

#[cfg(test)]
mod tests;

use std::borrow::Cow;

use lotus_model::ColumnarResultSet;

use crate::export::ExportFormat;

#[cfg(test)]
use self::{
    csv::csv_field,
    json::json_string,
    turtle::{number, render_mass},
};

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
    /// The rows not yet written. A range rather than a counter, so the next row
    /// comes from `next()` and no hand-written step can repeat or skip one.
    rows: std::ops::Range<usize>,
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
            rows: 0..set.row_count(),
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

            let Some(row) = self.rows.next() else {
                self.push_epilogue();
                self.done = true;
                return if self.chunk.is_empty() {
                    None
                } else {
                    Some(self.take())
                };
            };

            self.push_row(row);

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

    /// Swap the buffered chunk out and preallocate the next chunk buffer.
    fn take(&mut self) -> String {
        std::mem::replace(
            &mut self.chunk,
            String::with_capacity(CHUNK_TARGET + CHUNK_TARGET / 8),
        )
    }

    fn push_preamble(&mut self) {
        match self.format {
            ExportFormat::Csv => csv::push_preamble(&mut self.chunk, &COLUMNS),
            ExportFormat::Json => json::push_preamble(&mut self.chunk, &COLUMNS),
            ExportFormat::Rdf => turtle::push_preamble(&mut self.chunk),
        }
    }

    fn push_epilogue(&mut self) {
        match self.format {
            ExportFormat::Json => json::push_epilogue(&mut self.chunk),
            // Nothing for CSV: the header and every row already end in a newline.
            // A second one left a blank last line, which some readers take for an
            // empty record and which made `wc -l` count one line more than there
            // are. Turtle needs no closing either.
            ExportFormat::Csv | ExportFormat::Rdf => {}
        }
    }

    fn push_row(&mut self, row: usize) {
        let set = self.set;
        match self.format {
            ExportFormat::Csv => csv::push_row(&mut self.chunk, Self::cells(set, row)),
            ExportFormat::Json => {
                json::push_row(&mut self.chunk, row, &COLUMNS, Self::cells(set, row));
            }
            ExportFormat::Rdf => turtle::push_triples(&mut self.chunk, set, row),
        }
    }

    /// The row's cells, in [`COLUMNS`] order, as borrowed where possible.
    ///
    /// `Cow` because four of the thirteen are rendered rather than stored: the QIDs come out of
    /// a numeric dictionary and the statement out of packed bytes, so they need a buffer
    /// before they can be quoted. Borrowing keeps the other nine off the heap.
    fn cells(set: &ColumnarResultSet, row: usize) -> [Cow<'_, str>; COLUMNS.len()] {
        [
            render_qid(set.compound_qid_text(row)),
            Cow::Borrowed(set.compound_label(row).unwrap_or_default()),
            borrowed_or_empty(set.inchikey(row)),
            borrowed_or_empty(set.smiles(row)),
            Cow::Owned(turtle::render_mass(set.mass(row))),
            borrowed_or_empty(set.formula(row)),
            render_qid(set.taxon_qid_text(row)),
            borrowed_or_empty(set.taxon_label(row)),
            render_qid(set.reference_qid_text(row)),
            borrowed_or_empty(set.reference_title(row)),
            borrowed_or_empty(set.reference_doi(row)),
            Cow::Owned(
                set.pub_year(row)
                    .map_or_else(String::new, |year| year.to_string()),
            ),
            Cow::Owned(statement_text(set, row)),
        ]
    }
}

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

fn statement_text(set: &ColumnarResultSet, row: usize) -> String {
    set.statement_text(row).unwrap_or_default()
}
