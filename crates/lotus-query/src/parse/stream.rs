// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Reading a whole result set without holding the payload.
//!
//! # Why not the `csv` reader
//!
//! The [`csv`] crate reads a `Read` incrementally already, but it reads to end
//! of *input* to know it is done. That is fine for a file and wrong for a
//! response body arriving in chunks: there is no `Read` that can say "not yet,
//! ask me again later", and a reader that returns zero bytes is end of input to
//! `csv` and would truncate the result at the first chunk boundary.
//!
//! So the record splitting is done here, one chunk at a time, with an unfinished
//! record carried across the boundary. The rules are RFC 4180's -- quoted
//! fields, `""` for a literal quote, newlines inside quotes -- because the
//! reference titles in this dataset contain commas, quotes and the occasional
//! newline, and a splitter that mishandles them would silently shift every
//! column to the right of them.
//!
//! The parsing is deliberately as forgiving as
//! [`parse_compounds_csv_capped`](super::parse_compounds_csv_capped): a short
//! row reads as missing trailing fields rather than failing, because a search
//! that has already returned usable rows should not be discarded over one
//! malformed cell.

use crate::error::ParseError;
use lotus_model::{ColumnarBuilder, ColumnarResultSet, RawRow};
use std::io::Read;

use super::Columns;

/// One CSV record: the raw bytes of each field, unescaped.
type Record = Vec<Vec<u8>>;

/// Splits a CSV byte stream into records, across chunk boundaries.
///
/// ```
/// use lotus_query::CsvSplitter;
///
/// let mut splitter = CsvSplitter::new();
/// let mut records = Vec::new();
/// splitter.feed(b"a,b\nc,d\n", &mut records);
/// assert_eq!(records.len(), 2);
///
/// // A record split across two chunks is one record, not two.
/// let mut splitter = CsvSplitter::new();
/// let mut records = Vec::new();
/// splitter.feed(b"a,\"b", &mut records);
/// assert!(records.is_empty());
/// splitter.feed(b",c\"\n", &mut records);
/// assert_eq!(records.len(), 1);
/// ```
#[derive(Debug, Default)]
pub struct CsvSplitter {
    /// Bytes of the record being assembled.
    line: Vec<u8>,
    /// Fields of the record being assembled.
    fields: Vec<Vec<u8>>,
    /// Whether the cursor is inside a quoted field.
    in_quotes: bool,
    /// Whether a `"` was seen inside a quoted field and not yet resolved.
    ///
    /// A `""` inside a quoted field is one literal quote and a lone `"` ends the
    /// field, and the two are told apart by what follows. Deciding needs a
    /// lookahead, which a chunk boundary does not allow, so the decision waits
    /// for the next byte -- and if the payload ends first, the field ends too.
    quote_pending: bool,
}

impl CsvSplitter {
    /// An empty splitter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a record is part-read and so the next chunk continues it.
    #[must_use]
    pub const fn is_mid_record(&self) -> bool {
        !self.fields.is_empty() || !self.line.is_empty() || self.quote_pending
    }

    /// Split `chunk`, appending every record it completes to `out`.
    ///
    /// `out` is appended to rather than replaced, so a caller can reuse one
    /// vector for the lifetime of the stream and drain it after each chunk.
    pub fn feed(&mut self, chunk: &[u8], out: &mut Vec<Record>) {
        for &byte in chunk {
            if self.quote_pending {
                self.quote_pending = false;
                if byte == b'"' {
                    // `""` inside a quoted field: one literal quote, and the
                    // field is still open.
                    self.line.push(b'"');
                    continue;
                }
                // The pending quote closed the field, so this byte is outside it.
                self.in_quotes = false;
            }

            if self.in_quotes {
                if byte == b'"' {
                    self.quote_pending = true;
                } else {
                    self.line.push(byte);
                }
                continue;
            }

            match byte {
                b'"' if self.line.is_empty() => self.in_quotes = true,
                b',' => self.end_field(),
                b'\n' => self.end_record(out),
                // A CR is dropped: an endpoint that ends lines with CRLF would
                // otherwise leave a stray carriage return on the last field.
                b'\r' => {}
                _ => self.line.push(byte),
            }
        }
    }

    /// Finish the stream, flushing a record that was still being read.
    ///
    /// A payload that ends mid-record -- an unterminated quote, or a transport
    /// that cut the body short -- yields the record it has rather than dropping
    /// the row. A payload that ends cleanly flushes nothing.
    pub fn end_of_input(&mut self, out: &mut Vec<Record>) {
        self.quote_pending = false;
        if self.is_mid_record() {
            self.end_record(out);
        }
    }

    /// Close the current field.
    fn end_field(&mut self) {
        let field = std::mem::take(&mut self.line);
        self.fields.push(field);
    }

    /// Close the current record, appending it to `out`.
    fn end_record(&mut self, out: &mut Vec<Record>) {
        let last = std::mem::take(&mut self.line);
        self.fields.push(last);
        let record = std::mem::take(&mut self.fields);
        out.push(record);
        self.in_quotes = false;
        self.quote_pending = false;
    }
}

/// A record's field, trimmed, as UTF-8, or empty.
fn field_bytes(record: &Record, at: Option<usize>) -> &str {
    at.and_then(|i| record.get(i))
        .and_then(|b| std::str::from_utf8(b).ok())
        .map_or("", str::trim)
}

/// Turns a CSV stream into a [`ColumnarResultSet`], one chunk at a time.
///
/// The set is the only thing that grows: the payload is never assembled. Peak
/// memory is therefore the finished set plus one chunk, which is what makes a
/// three-million-row result fit a budget that the raw CSV could not.
#[derive(Debug, Default)]
pub struct CsvColumnarReader {
    splitter: CsvSplitter,
    builder: ColumnarBuilder,
    columns: Option<Columns>,
    header: Option<Vec<String>>,
    scratch: Vec<Record>,
    rows_read: usize,
    bytes_read: usize,
}

impl CsvColumnarReader {
    /// A reader that has seen nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed the next slice of the response body.
    ///
    /// Returns how many rows have been added to the set so far, so a caller can
    /// report progress without keeping its own count.
    ///
    /// # Errors
    /// Returns [`ParseError`] if a record cannot be decoded as UTF-8. A record
    /// that decodes but has the wrong number of fields is kept, with the missing
    /// ones empty.
    pub fn feed(&mut self, chunk: &[u8]) -> Result<usize, ParseError> {
        self.bytes_read += chunk.len();
        // The scratch buffer is moved out rather than borrowed, because
        // `read_header` and `push` both need `&mut self`. Its allocation comes
        // back afterwards, so it is reused for every chunk of the stream.
        let mut scratch = std::mem::take(&mut self.scratch);
        scratch.clear();
        self.splitter.feed(chunk, &mut scratch);
        self.absorb(scratch)?;
        Ok(self.rows_read)
    }

    /// Fold a batch of finished records into the set.
    fn absorb(&mut self, scratch: Vec<Record>) -> Result<(), ParseError> {
        for record in &scratch {
            if self.columns.is_none() {
                self.read_header(record);
                continue;
            }
            let columns = self
                .columns
                .as_ref()
                .ok_or_else(|| ParseError::new("a record arrived before the header row"))?;
            self.rows_read += 1;
            self.builder.push(raw_row(record, columns));
        }
        self.scratch = scratch;
        Ok(())
    }

    /// Take the header row, which names the columns.
    ///
    /// A missing header is not an error: every column then reads as absent and
    /// the set comes back empty, which is the same forgiving outcome the
    /// row-at-a-time parser reaches.
    fn read_header(&mut self, record: &Record) {
        self.header = Some(
            record
                .iter()
                .map(|field| String::from_utf8_lossy(field).trim().to_owned())
                .collect(),
        );
        let names = self.header.as_ref().map_or_else(Vec::new, Clone::clone);
        self.columns = Some(Columns::from_names(&names));
    }

    /// Refuse a payload whose header names no column this reader knows.
    ///
    /// Without this, a query and a parser that have drifted apart produce an
    /// empty set: indistinguishable from a search that found nothing, and far
    /// harder to notice than an error.
    fn check_header(&self) -> Result<(), ParseError> {
        let Some(names) = self.header.as_ref() else {
            return Err(ParseError::new("the response had no header row"));
        };
        if self.columns.is_none_or(|c| !c.resolves_any()) {
            return Err(ParseError::new(format!(
                "the response header names none of the expected columns; it had {names:?}"
            )));
        }
        Ok(())
    }

    /// How many rows have been added.
    #[must_use]
    pub const fn row_count(&self) -> usize {
        self.rows_read
    }

    /// How many bytes have been fed.
    #[must_use]
    pub const fn byte_count(&self) -> usize {
        self.bytes_read
    }

    /// The column names the endpoint sent, if it sent a header.
    #[must_use]
    pub fn header(&self) -> Option<&[String]> {
        self.header.as_deref()
    }

    /// Finish the set, computing its exact counts.
    ///
    /// # Errors
    /// Returns [`ParseError`] if the payload held no header row, because then
    /// no column was ever identified and every row would be silently empty.
    pub fn finish(mut self) -> Result<ColumnarResultSet, ParseError> {
        let mut scratch = std::mem::take(&mut self.scratch);
        scratch.clear();
        self.splitter.end_of_input(&mut scratch);
        self.absorb(scratch)?;
        self.check_header()?;
        Ok(self.builder.build())
    }
}

/// The size of one read from a native reader.
///
/// Small enough that the payload never accumulates in a read buffer, large enough
/// that a million reads is not a million syscalls.
const READ_BUFFER: usize = 64 * 1024;

/// Read a whole CSV payload into a columnar set, streaming it.
///
/// # Errors
/// Returns [`ParseError`] if the payload cannot be read, or has no header row.
pub fn parse_compounds_columnar<R: Read>(mut reader: R) -> Result<ColumnarResultSet, ParseError> {
    let mut columnar = CsvColumnarReader::new();
    let mut buffer = vec![0u8; READ_BUFFER];
    loop {
        let read = reader.read(&mut buffer).map_err(ParseError::new)?;
        if read == 0 {
            break;
        }
        let chunk = buffer.get(..read).unwrap_or_default();
        columnar.feed(chunk)?;
    }
    columnar.finish()
}

/// One record as a [`RawRow`].
fn raw_row<'a>(record: &'a Record, columns: &'a Columns) -> RawRow<'a> {
    let iso = field_bytes(record, columns.smiles_iso);
    let conn = field_bytes(record, columns.smiles_conn);
    let smiles = if iso.is_empty() { conn } else { iso };

    RawRow {
        compound_qid: field_bytes(record, columns.compound),
        name: field_bytes(record, columns.label),
        inchikey: optional(field_bytes(record, columns.inchikey)),
        smiles: optional(smiles),
        mass: field_bytes(record, columns.mass).parse().ok(),
        formula: optional(field_bytes(record, columns.formula)),
        taxon_qid: field_bytes(record, columns.taxon),
        taxon_name: field_bytes(record, columns.taxon_name),
        reference_qid: field_bytes(record, columns.reference),
        ref_title: optional(field_bytes(record, columns.ref_title)),
        ref_doi: optional(field_bytes(record, columns.ref_doi)),
        pub_year: field_bytes(record, columns.ref_date)
            .split(['-', 'T'])
            .next()
            .and_then(|year| year.parse().ok()),
        statement: optional(field_bytes(record, columns.statement)),
    }
}

/// An empty string as an absent value.
const fn optional(value: &str) -> Option<&str> {
    if value.is_empty() { None } else { Some(value) }
}
