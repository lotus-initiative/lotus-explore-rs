// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Single-row `COUNT(...)` CSV response parsing.

use lotus_model::DatasetStats;

use super::columns::open_csv_reader;
use crate::error::ParseError;

/// Read the single-row `COUNT` result.
///
/// A reported `n_entries_unique` of zero means the count subquery did not
/// evaluate, not that every row is a duplicate; reporting the raw count is the
/// lesser wrong answer.
///
/// # Errors
/// Returns [`ParseError`] if the payload has no data row, because there
/// is then nothing to fall back on: a total cannot be invented.
pub fn parse_counts_csv(bytes: &[u8]) -> Result<DatasetStats, ParseError> {
    let mut reader = open_csv_reader(bytes);

    let headers = reader.headers().map_err(ParseError::new)?.clone();
    let index = |name: &str| headers.iter().position(|h| h == name);

    let mut records = reader.records();
    let record = match records.next() {
        Some(Ok(r)) => r,
        Some(Err(e)) => return Err(ParseError::new(e)),
        None => return Err(ParseError::new("the count query returned no row")),
    };
    let read_optional = |name: &str| -> Result<usize, ParseError> {
        let Some(col) = index(name) else {
            return Ok(0);
        };
        let raw = record.get(col).map_or_default(str::trim);
        if raw.is_empty() {
            return Ok(0);
        }
        raw.parse::<usize>()
            .map_err(|_| ParseError::new(format!("invalid count value for {name}: {raw:?}")))
    };

    let Some(entries_col) = index("n_entries") else {
        return Err(ParseError::new(
            "the count query response is missing n_entries",
        ));
    };
    let entries_raw = record.get(entries_col).map_or_default(str::trim);
    let n_entries = entries_raw.parse::<usize>().map_err(|_| {
        ParseError::new(format!(
            "invalid count value for n_entries: {entries_raw:?}"
        ))
    })?;
    let reported_unique = read_optional("n_entries_unique")?;

    Ok(DatasetStats {
        n_entries,
        n_entries_unique: if reported_unique == 0 {
            n_entries
        } else {
            reported_unique
        },
        n_compounds: read_optional("n_compounds")?,
        n_taxa: read_optional("n_taxa")?,
        n_references: read_optional("n_references")?,
    })
}
