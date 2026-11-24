// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Buffered 15-column compound CSV readers with optional row caps.

use lotus_model::{CompoundEntry, DatasetStats};

use super::stream::parse_compounds_columnar;
use crate::error::ParseError;

/// Parse rows into entries, through the one result reader.
///
/// # Errors
/// Returns [`ParseError`] if the payload cannot be read as CSV. A payload
/// that reads but has the wrong shape does not: missing columns become empty.
pub fn parse_compounds_csv(
    bytes: &[u8],
    max_rows: usize,
) -> Result<Vec<CompoundEntry>, ParseError> {
    let (rows, _, _) = parse_compounds_csv_capped(bytes, max_rows)?;
    Ok(rows)
}

/// Parse rows and count the whole result set, whether or not the rows were kept.
///
/// This is the reader to prefer: the counts it returns are the endpoint's, and
/// `capped` says whether the display limit hid part of the result. A caller
/// that counts from the returned rows cannot reproduce the raw count, because
/// deduplication happens during parsing.
///
/// # Errors
/// Returns [`ParseError`] if the payload cannot be read as CSV.
pub fn parse_compounds_csv_capped(
    bytes: &[u8],
    max_rows: usize,
) -> Result<(Vec<CompoundEntry>, DatasetStats, bool), ParseError> {
    // An empty body is an empty result, not a malformed one.
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok((Vec::new(), DatasetStats::default(), false));
    }

    let set = parse_compounds_columnar(bytes)?;

    let count = set.row_count();
    let limit = count.min(max_rows);
    let mut rows = Vec::with_capacity(limit.min(2048));
    for row in 0..limit {
        if let Some(entry) = set.entry(row) {
            rows.push(entry);
        }
    }

    let truncated = count > rows.len();

    Ok((rows, set.stats(), truncated))
}
