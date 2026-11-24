// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Incremental RFC 4180 CSV record splitter across arbitrary byte chunks.

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
    /// Bytes of the field being assembled.
    line: Vec<u8>,
    /// Reusable field buffers for the record being assembled.
    fields: Vec<Vec<u8>>,
    /// How many fields of `fields` belong to the current record.
    field_count: usize,
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
        self.field_count != 0 || !self.line.is_empty() || self.quote_pending
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
        self.feed_with(chunk, |record| out.push(record.to_vec()));
    }

    /// Split `chunk`, invoking `on_record` with a borrowed slice of reusable field buffers
    /// as each record completes.
    pub fn feed_with(&mut self, chunk: &[u8], mut on_record: impl FnMut(&[Vec<u8>])) {
        // The unread part of the chunk shrinks by destructuring, never by cursor
        // arithmetic: each pass either consumes at least the byte that stopped
        // the run or leaves the loop, so no step size can stall it.
        let mut rest = chunk;
        while !rest.is_empty() {
            if self.quote_pending {
                // A `""` inside a quoted field is one literal quote; anything else
                // means the pending quote closed the field and this byte is outside
                // it. Deciding needs the lookahead, which is what this flag is for.
                self.quote_pending = false;
                if let Some(after) = rest.strip_prefix(b"\"") {
                    self.line.push(b'"');
                    rest = after;
                    continue;
                }
                self.in_quotes = false;
            }

            // Split inline rather than in a helper returning the rest: a helper can be
            // swapped for one whose rest never shrinks, and then this loop never ends.
            let stop = if self.in_quotes {
                rest.iter().position(|byte| *byte == b'"')
            } else {
                rest.iter().position(|byte| is_delimiter(*byte))
            };
            let Some((run, tail)) = stop.and_then(|at| rest.split_at_checked(at)) else {
                self.line.extend_from_slice(rest);
                break;
            };
            self.line.extend_from_slice(run);
            let Some((&delimiter, after)) = tail.split_first() else {
                break;
            };
            rest = after;
            if self.in_quotes {
                self.quote_pending = true;
                continue;
            }
            match delimiter {
                // A quote only opens a field at its start; one in the middle of a
                // field is malformed input and is kept as a literal character.
                b'"' if self.line.is_empty() => self.in_quotes = true,
                b',' => self.end_field(),
                b'\n' => self.end_record(&mut on_record),
                // A CR is dropped: an endpoint that ends lines with CRLF would
                // otherwise leave a stray carriage return on the last field.
                b'\r' => {}
                _ => self.line.push(delimiter),
            }
        }
    }

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
        self.end_of_input_with(|record| out.push(record.to_vec()))
    }

    /// End the stream, flushing any unfinished record to `on_record`.
    pub fn end_of_input_with(&mut self, mut on_record: impl FnMut(&[Vec<u8>])) -> bool {
        // `in_quotes && !quote_pending`, not `in_quotes`. The flag is the difference: a `"`
        // inside a quoted field leaves the decision pending until the next byte says whether
        // it closed the field or escaped a literal quote, so a field closed by the very last
        // byte is still in-quotes with a pending quote -- a complete row. An *unresolved*
        // flag is the opposite: the quote opened and nothing closed it.
        let truncated = self.in_quotes && !self.quote_pending;
        self.quote_pending = false;
        if self.is_mid_record() {
            self.end_record(&mut on_record);
        }
        truncated
    }

    /// Close the current field, reusing the existing slot buffer when present.
    fn end_field(&mut self) {
        if let Some(slot) = self.fields.get_mut(self.field_count) {
            std::mem::swap(&mut self.line, slot);
            self.line.clear();
        } else {
            self.fields.push(std::mem::take(&mut self.line));
        }
        self.field_count += 1;
    }

    /// Close the current record and hand its active fields to `on_record`.
    fn end_record(&mut self, on_record: &mut impl FnMut(&[Vec<u8>])) {
        self.end_field();
        let count = self.field_count;
        self.field_count = 0;
        self.in_quotes = false;
        self.quote_pending = false;
        on_record(self.fields.get(..count).unwrap_or_default());
    }
}

/// Whether a byte is a delimiter, indexed by value.
///
/// A table rather than four comparisons, so the scan in [`is_delimiter`] is one
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

/// Whether `byte` ends or escapes a field outside any quoted run.
fn is_delimiter(byte: u8) -> bool {
    DELIMITERS.get(usize::from(byte)).copied().unwrap_or(false)
}

#[cfg(test)]
#[path = "splitter/tests.rs"]
mod tests;
