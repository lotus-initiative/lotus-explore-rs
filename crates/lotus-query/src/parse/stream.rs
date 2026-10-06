// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Reading a whole result set without holding the payload.
//!
//! # Why not the `csv` reader
//!
//! The [`csv`] crate reads a `Read` incrementally, but reads to end of *input* to know it
//! is done. Fine for a file, wrong for a body arriving in chunks: no `Read` can say "not
//! yet, ask me again", and zero bytes is end of input to `csv`, truncating the result at
//! the first chunk boundary.
//!
//! So record splitting is done here, one chunk at a time, an unfinished record carried
//! across the boundary. The rules are RFC 4180's -- quoted fields, `""` for a literal
//! quote, newlines inside quotes -- because reference titles here contain commas, quotes
//! and occasional newlines, and mishandling them silently shifts every column to the right.
//!
//! Parsing is as forgiving as
//! [`parse_compounds_csv_capped`](super::parse_compounds_csv_capped): a short row reads as
//! missing trailing fields rather than failing, since a search that returned usable rows
//! should not be discarded over one malformed cell.

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
    /// `out` is appended to rather than replaced, so a caller can reuse one vector for the
    /// life of the stream and drain it after each chunk.
    ///
    /// Ordinary bytes are copied a *run* at a time. The obvious per-byte loop spent 2.5s of
    /// a 1M-row build against ~0.4s for this, paying a capacity check and a bounds check
    /// for each of 291 million bytes. Nothing needs inspecting between delimiters, so the
    /// run crosses in one `extend_from_slice` and per-byte work happens only at a
    /// delimiter.
    pub fn feed(&mut self, chunk: &[u8], out: &mut Vec<Record>) {
        let mut at = 0;
        while at < chunk.len() {
            if self.quote_pending {
                // A `""` inside a quoted field is one literal quote; anything else
                // means the pending quote closed the field and this byte is outside
                // it. Deciding needs the lookahead, which is what this flag is for.
                let byte = chunk.get(at).copied().unwrap_or_default();
                self.quote_pending = false;
                if byte == b'"' {
                    self.line.push(b'"');
                    at += 1;
                    continue;
                }
                self.in_quotes = false;
            }

            if self.in_quotes {
                let rest = chunk.get(at..).unwrap_or_default();
                if let Some(offset) = find_byte(rest, b'"') {
                    self.line
                        .extend_from_slice(rest.get(..offset).unwrap_or_default());
                    at += offset + 1;
                    self.quote_pending = true;
                } else {
                    self.line.extend_from_slice(rest);
                    at = chunk.len();
                }
                continue;
            }

            let rest = chunk.get(at..).unwrap_or_default();
            let Some(offset) = find_delimiter(rest) else {
                self.line.extend_from_slice(rest);
                break;
            };
            self.line
                .extend_from_slice(rest.get(..offset).unwrap_or_default());
            let delimiter = rest.get(offset).copied().unwrap_or_default();
            at += offset + 1;
            match delimiter {
                // A quote only opens a field at its start; one in the middle of a
                // field is malformed input and is kept as a literal character.
                b'"' if self.line.is_empty() => self.in_quotes = true,
                b',' => self.end_field(),
                b'\n' => self.end_record(out),
                // A CR is dropped: an endpoint that ends lines with CRLF would
                // otherwise leave a stray carriage return on the last field.
                b'\r' => {}
                _ => self.line.push(delimiter),
            }
        }
    }

    /// Finish the stream, flushing a record that was still being read.
    ///
    /// A payload that ends mid-record -- an unterminated quote, or a transport that cut the body
    /// short -- yields the record it has rather than dropping the row. A clean end flushes
    /// nothing.
    /// End the stream, flushing whatever the last bytes left open.
    ///
    /// Returns `true` if the input stopped inside a quoted field, the one truncation that is
    /// unambiguously detectable.
    ///
    /// That case matters because it used to pass for data: a response cut short by a timeout,
    /// proxy or cancelled query can stop anywhere, and stopping inside a quoted field closed
    /// the partial record and handed it over like any other. A short result set is then
    /// indistinguishable from a query that genuinely matched fewer rows, and every count,
    /// hash and export built on it describes rows that were never returned.
    ///
    /// What cannot be detected, exactly: a body stopping *between* fields, or after a row's
    /// last field with no trailing newline, is byte-for-byte identical to a complete final
    /// row. CSV allows the last record to omit its terminator, so calling that truncation
    /// would reject well-formed responses from an endpoint that does not end with a newline.
    /// Only the unterminated quote is unambiguous, and it is the common shape for a cut
    /// inside a title or SMILES -- the columns most likely mid-field when a response dies.
    pub fn end_of_input(&mut self, out: &mut Vec<Record>) -> bool {
        // `in_quotes && !quote_pending`, not `in_quotes`. The flag is the difference: a `"`
        // inside a quoted field leaves the decision pending until the next byte says whether
        // it closed the field or escaped a literal quote, so a field closed by the very last
        // byte is still in-quotes with a pending quote -- a complete row. An *unresolved*
        // flag is the opposite: the quote opened and nothing closed it.
        let truncated = self.in_quotes && !self.quote_pending;
        self.quote_pending = false;
        if self.is_mid_record() {
            self.end_record(out);
        }
        truncated
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

/// Whether a byte is a delimiter, indexed by value.
///
/// A table rather than four comparisons, so the scan in [`find_delimiter`] is one
/// branch per byte. A 256-entry table is 256 bytes of the module's data, which is
/// worth more than the branch it saves on a 291-megabyte payload.
#[allow(
    clippy::indexing_slicing,
    reason = "the four indices are literals inside a 256-entry array; the alternative \
              is a `const fn`, and `slice::get` is not `const` on this toolchain"
)]
static DELIMITERS: [bool; 256] = {
    let mut table = [false; 256];
    table[34] = true; // `"`
    table[44] = true; // `,`
    table[10] = true; // `\n`
    table[13] = true; // `\r`
    table
};

/// The first `needle` in `haystack`.
fn find_byte(haystack: &[u8], needle: u8) -> Option<usize> {
    haystack.iter().position(|byte| *byte == needle)
}

/// The first byte that ends or escapes a field, outside any quoted run.
fn find_delimiter(haystack: &[u8]) -> Option<usize> {
    haystack
        .iter()
        .position(|byte| DELIMITERS.get(*byte as usize).copied().unwrap_or(false))
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
    /// The last [`TRUNCATION_NOTICE_WINDOW`] bytes of the body, for the notice.
    ///
    /// Kept rather than searched for as the stream arrives, because the notice is
    /// written at the very end and can straddle a chunk boundary: a body cut
    /// between the marker and its reason would otherwise read as complete.
    tail: Vec<u8>,
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
        self.keep_tail(chunk);
        // The scratch buffer is moved out rather than borrowed, because
        // `read_header` and `push` both need `&mut self`. Its allocation comes
        // back afterwards, so it is reused for every chunk of the stream.
        let mut scratch = std::mem::take(&mut self.scratch);
        scratch.clear();
        self.splitter.feed(chunk, &mut scratch);
        self.absorb(scratch)?;
        Ok(self.rows_read)
    }

    /// Extend the tail window with `chunk`, keeping only the last window's worth.
    ///
    /// The window is a constant size, so the tail is never longer than a chunk
    /// plus the window; taking the last `WINDOW` bytes of the concatenation is
    /// therefore always correct and never allocates more than that.
    fn keep_tail(&mut self, chunk: &[u8]) {
        self.tail.extend_from_slice(chunk);
        let excess = self.tail.len().saturating_sub(TRUNCATION_NOTICE_WINDOW);
        if excess > 0 {
            self.tail.drain(..excess);
        }
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
    /// Finish the body, or refuse it if it stopped mid-record.
    ///
    /// # Errors
    /// [`ParseError`] if the endpoint appended its truncation notice, if the response
    /// ended inside a quoted field, or if there is no header row.
    /// A truncated body is refused rather than parsed: the rows before the cut are
    /// valid, and returning them would present an incomplete answer as a complete one.
    pub fn finish(mut self) -> Result<ColumnarResultSet, ParseError> {
        let mut scratch = std::mem::take(&mut self.scratch);
        scratch.clear();
        if self.splitter.end_of_input(&mut scratch) {
            return Err(ParseError::new(INCOMPLETE_BODY));
        }
        self.absorb(scratch)?;
        self.check_header()?;
        if let Some(reason) = truncation_notice(&self.tail) {
            return Err(ParseError::new(truncated_by_endpoint(reason)));
        }
        Ok(self.builder.build())
    }
}

/// How much of the body's end is kept, to look for the notice in.
///
/// The notice is a few hundred bytes, so a kilobyte is generous. Kept as a
/// constant rather than a literal in the slice because the whole point is that
/// this window is sized to the notice, and a window that silently stopped
/// covering it would make truncation undetectable again.
const TRUNCATION_NOTICE_WINDOW: usize = 1024;

/// What `QLever` appends to a result it could not finish sending.
///
/// It has no other way to say so: the transfer is already committed to HTTP, so
/// the error is written into the body of an otherwise-`200` response. The exact
/// wording is theirs, and what is matched is the stable marker rather than the
/// prose around it.
const TRUNCATION_NOTICE_MARKER: &[u8] = b"!!!!>>#";

/// The reasons `QLever` names, matched case-insensitively and without the period.
const TRUNCATION_REASONS: &[&str] = &["Operation timed out", "Query was canceled"];

/// The reason the endpoint gave, if the body ends in its truncation notice.
///
/// Returns `None` for a complete answer, which is the case this exists to
/// distinguish.
fn truncation_notice(tail: &[u8]) -> Option<&'static str> {
    let at = tail
        .windows(TRUNCATION_NOTICE_MARKER.len())
        .rposition(|window| window == TRUNCATION_NOTICE_MARKER)?;
    // `at` came from `rposition` over `windows` of the same length, so it is a
    // valid boundary; `get` rather than a slice because this crate forbids
    // indexing on external input and the compiler cannot see that invariant.
    let text = String::from_utf8_lossy(tail.get(at..)?);
    // The reason is on its own line after the prose, and is worth keeping: it is
    // the only account of *why* the answer stopped, and "timed out" and "cancelled"
    // read very differently to someone whose query keeps dying.
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    lines.iter().find_map(|line| {
        let line = line.strip_suffix('.').unwrap_or(line);
        TRUNCATION_REASONS
            .iter()
            .find(|reason| line.eq_ignore_ascii_case(reason))
            .copied()
    })
}

/// The message a truncation produces, carrying the endpoint's own reason.
fn truncated_by_endpoint(reason: &'static str) -> String {
    format!(
        "the endpoint stopped sending before the result was complete, so these rows \
         are only part of the answer and are not being reported as if they were all \
         of it (the endpoint said: {reason}); re-run the search, or narrow it"
    )
}

/// The message a truncated body produces.
///
/// Named rather than inlined because a caller may want to match on it, and because the
/// wording is the only thing standing between "the endpoint was slow" and a result set
/// that is quietly wrong.
pub(super) const INCOMPLETE_BODY: &str = "the response ended in the middle of a row, so the result set is incomplete; \
     this usually means the endpoint timed out and the rows below are missing";

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
        reference_node: super::normalize_reference_node(field_bytes(
            record,
            columns.reference_node,
        ))
        .unwrap_or_default(),
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
