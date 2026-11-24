// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Chunked [`CsvColumnarReader`] folding occurrence and enrichment streams into [`ColumnarResultSet`].

use lotus_model::{ColumnarBuilder, ColumnarResultSet};

use super::fields::{
    INCOMPLETE_BODY, TRUNCATION_NOTICE_WINDOW, raw_row, truncated_by_endpoint, truncation_notice,
};
use super::splitter::CsvSplitter;
use crate::error::ParseError;
use crate::parse::Columns;

/// Turns a CSV stream into a [`ColumnarResultSet`], one chunk at a time.
///
/// The set is the only thing that grows: the payload is never assembled. Peak
/// memory is therefore the finished set plus one chunk, which is what makes a
/// three-million-row result fit a budget that the raw CSV could not.
#[derive(Debug, Default)]
pub struct CsvColumnarReader {
    splitter: CsvSplitter,
    builder: ColumnarBuilder,
    extra_entities: ColumnarBuilder,
    columns: Option<Columns>,
    header: Option<Vec<String>>,
    rows_read: usize,
    bytes_read: usize,
    enrich_only: bool,
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
        let columns = &mut self.columns;
        let header = &mut self.header;
        let rows_read = &mut self.rows_read;
        let builder = &mut self.builder;
        let extra_entities = &mut self.extra_entities;
        let enrich_only = self.enrich_only;
        self.splitter.feed_with(chunk, |record| {
            Self::absorb_record(
                columns,
                header,
                rows_read,
                builder,
                extra_entities,
                enrich_only,
                record,
            );
        });
        Ok(self.rows_read)
    }

    /// Extend the tail window with `chunk`, keeping only the last window's worth.
    ///
    /// The window is a constant size, so the tail is never longer than a chunk
    /// plus the window; taking the last `WINDOW` bytes of the concatenation is
    /// therefore always correct and never allocates more than that.
    fn keep_tail(&mut self, chunk: &[u8]) {
        self.tail.extend_from_slice(chunk);
        // Unconditional: draining an empty range is free, and a guard here would
        // only be a second statement of the same bound.
        let excess = self.tail.len().saturating_sub(TRUNCATION_NOTICE_WINDOW);
        self.tail.drain(..excess);
    }

    /// Fold one finished record into the set, or capture it as the header row.
    fn absorb_record(
        columns: &mut Option<Columns>,
        header: &mut Option<Vec<String>>,
        rows_read: &mut usize,
        builder: &mut ColumnarBuilder,
        extra_entities: &mut ColumnarBuilder,
        enrich_only: bool,
        record: &[Vec<u8>],
    ) {
        if let Some(cols) = columns.as_ref() {
            let row = raw_row(record, cols);
            if enrich_only {
                if !builder.enrich(row) {
                    extra_entities.intern_dictionary_row(row);
                }
            } else {
                *rows_read += 1;
                builder.push(row);
            }
        } else {
            // `trim` keeps a byte-order mark, which would rename the first column
            // and lose every row for want of a `compound`.
            let names: Vec<String> = record
                .iter()
                .map(|field| {
                    String::from_utf8_lossy(field)
                        .trim()
                        .trim_start_matches('\u{feff}')
                        .to_owned()
                })
                .collect();
            *columns = Some(Columns::from_names(&names));
            *header = Some(names);
        }
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

    /// Whether the occurrence stream omitted compound and reference metadata
    /// columns, so dictionary enrichment streams are needed.
    #[must_use]
    pub fn needs_enrichment(&self) -> bool {
        self.columns.is_some_and(|c| c.label.is_none())
    }

    /// Pre-populate compound and reference metadata for any entity already
    /// present in `previous`, so dictionary enrichment only queries the
    /// endpoint for entities not yet seen.
    pub fn seed_from(&mut self, previous: &ColumnarResultSet) {
        self.builder.seed_from(previous);
    }

    /// Every distinct compound QID interned so far that still needs enrichment,
    /// in first-seen order.
    pub fn compound_qids(&self) -> impl Iterator<Item = u32> + '_ {
        self.builder.compound_qids()
    }

    /// Every distinct reference QID interned so far that still needs enrichment,
    /// in first-seen order.
    pub fn reference_qids(&self) -> impl Iterator<Item = u32> + '_ {
        self.builder.reference_qids()
    }

    /// Any dictionary entities that arrived during an unconstrained enrichment
    /// stream and did not belong to this result set's own occurrences, built
    /// into a seed-only [`ColumnarResultSet`] for caching across searches.
    #[must_use]
    pub fn take_extra_entities(&mut self) -> Option<ColumnarResultSet> {
        let extra = std::mem::take(&mut self.extra_entities);
        (!extra.is_empty_dictionaries()).then(|| extra.build())
    }

    /// Finish the current stream and reset the CSV state so the next stream
    /// enriches the dictionaries already interned in `builder`.
    ///
    /// # Errors
    /// Returns [`ParseError`] if the current stream ended in a truncation notice,
    /// an unclosed quote, or without a recognized header.
    pub fn next_enrichment_stream(&mut self) -> Result<(), ParseError> {
        self.validate_stream_end()?;
        self.splitter = CsvSplitter::default();
        self.columns = None;
        self.header = None;
        self.tail.clear();
        self.enrich_only = true;
        Ok(())
    }

    fn validate_stream_end(&mut self) -> Result<(), ParseError> {
        // Check for the endpoint's inline truncation notice before reporting an
        // open quoted field: when QLever times out mid-cell inside a quoted
        // label or title, the unclosed `"` stays open through the appended
        // `!!!!>># ... Operation timed out.` trailer, and `INCOMPLETE_BODY`
        // would otherwise mask the endpoint's explicit timeout reason.
        if let Some(reason) = truncation_notice(&self.tail) {
            return Err(ParseError::new(truncated_by_endpoint(reason)));
        }
        let columns = &mut self.columns;
        let header = &mut self.header;
        let rows_read = &mut self.rows_read;
        let builder = &mut self.builder;
        let extra_entities = &mut self.extra_entities;
        let enrich_only = self.enrich_only;
        if self.splitter.end_of_input_with(|record| {
            Self::absorb_record(
                columns,
                header,
                rows_read,
                builder,
                extra_entities,
                enrich_only,
                record,
            );
        }) {
            return Err(ParseError::new(INCOMPLETE_BODY));
        }
        self.check_header()
    }

    /// Finish the set, computing its exact counts.
    ///
    /// # Errors
    /// Returns [`ParseError`] if the payload held no header row, if the endpoint
    /// appended its truncation notice, or if the response ended inside a quoted
    /// field. A truncated body is refused rather than parsed: the rows before the
    /// cut are valid, and returning them would present an incomplete answer as a
    /// complete one.
    pub fn finish(mut self) -> Result<ColumnarResultSet, ParseError> {
        self.validate_stream_end()?;
        Ok(self.builder.build())
    }
}
