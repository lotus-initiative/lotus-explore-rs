// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Record field projection, truncation notice detection, and synchronous `Read` helper.

use std::io::Read;

use lotus_model::{ColumnarResultSet, RawRow};

use super::reader::CsvColumnarReader;
use crate::error::ParseError;
use crate::parse::{Columns, normalize_reference_node};

/// How much of the body's end is kept, to look for the notice in.
///
/// The notice is a few hundred bytes, so a kilobyte is generous. Kept as a
/// constant rather than a literal in the slice because the whole point is that
/// this window is sized to the notice, and a window that silently stopped
/// covering it would make truncation undetectable again.
pub(super) const TRUNCATION_NOTICE_WINDOW: usize = 1024;

/// What `QLever` appends to a result it could not finish sending.
const TRUNCATION_NOTICE_MARKER: &[u8] = b"!!!!>>#";

/// The reasons `QLever` names, matched case-insensitively and without the period.
const TRUNCATION_REASONS: &[&str] = &["Operation timed out", "Query was canceled"];

/// The message a truncated body produces.
///
/// Named rather than inlined because a caller may want to match on it, and because the
/// wording is the only thing standing between "the endpoint was slow" and a result set
/// that is quietly wrong.
pub(super) const INCOMPLETE_BODY: &str = "the response ended in the middle of a row, so the result set is incomplete; \
     this usually means the endpoint timed out and the rows below are missing";

/// The size of one read from a native reader.
///
/// 64 KiB: small enough that the payload never accumulates in a read buffer, large enough
/// that a million reads is not a million syscalls.
const READ_BUFFER: usize = 65_536;

/// The reason the endpoint gave, if the body ends in its truncation notice.
pub(super) fn truncation_notice(tail: &[u8]) -> Option<&'static str> {
    let at = tail
        .windows(TRUNCATION_NOTICE_MARKER.len())
        .rposition(|window| window == TRUNCATION_NOTICE_MARKER)?;
    let text = String::from_utf8_lossy(tail.get(at..)?);
    text.lines().map(str::trim).find_map(|line| {
        let line = line.strip_suffix('.').unwrap_or(line);
        TRUNCATION_REASONS
            .iter()
            .find(|reason| line.eq_ignore_ascii_case(reason))
            .copied()
    })
}

/// The message a truncation produces, carrying the endpoint's own reason.
pub(super) fn truncated_by_endpoint(reason: &'static str) -> String {
    format!(
        "the endpoint stopped sending before the result was complete, so these rows \
         are only part of the answer and are not being reported as if they were all \
         of it (the endpoint said: {reason}); re-run the search, or narrow it"
    )
}

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

/// A record's field, trimmed, as UTF-8, or empty.
fn field_bytes(record: &[Vec<u8>], at: Option<usize>) -> &str {
    at.and_then(|i| record.get(i))
        .and_then(|b| std::str::from_utf8(b).ok())
        .map_or("", str::trim)
}

/// One record as a [`RawRow`].
pub(super) fn raw_row<'a>(record: &'a [Vec<u8>], columns: &'a Columns) -> RawRow<'a> {
    let iso = field_bytes(record, columns.smiles_iso);
    let conn = field_bytes(record, columns.smiles_conn);
    let smiles = if iso.is_empty() { conn } else { iso };

    RawRow {
        compound_qid: field_bytes(record, columns.compound),
        name: field_bytes(record, columns.label),
        inchikey: optional(field_bytes(record, columns.inchikey)),
        smiles: optional(smiles),
        mass: optional(field_bytes(record, columns.mass)).and_then(|v| v.parse().ok()),
        formula: optional(field_bytes(record, columns.formula)),
        taxon_qid: field_bytes(record, columns.taxon),
        taxon_name: field_bytes(record, columns.taxon_name),
        reference_qid: field_bytes(record, columns.reference),
        reference_node: normalize_reference_node(field_bytes(record, columns.reference_node))
            .unwrap_or_default(),
        ref_title: optional(field_bytes(record, columns.ref_title)),
        ref_doi: optional(field_bytes(record, columns.ref_doi)),
        pub_year: optional(field_bytes(record, columns.ref_date))
            .and_then(|v| v.split(['-', 'T']).next())
            .and_then(|year| year.parse().ok()),
        statement: optional(field_bytes(record, columns.statement)),
    }
}

/// An empty string as an absent value.
const fn optional(value: &str) -> Option<&str> {
    if value.is_empty() { None } else { Some(value) }
}
