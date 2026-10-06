// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::curation::{CurationInputRow, row_uniqueness_key};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppendOutcome {
    pub added: usize,
    pub skipped: usize,
}

pub fn non_empty_trimmed(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.into())
    }
}

pub fn append_unique_rows(
    queue: &mut Vec<CurationInputRow>,
    candidates: impl IntoIterator<Item = CurationInputRow>,
) -> AppendOutcome {
    let mut seen = queue
        .iter()
        .map(row_uniqueness_key)
        .collect::<HashSet<String>>();

    let mut added = 0usize;
    let mut skipped = 0usize;

    for row in candidates {
        let key = row_uniqueness_key(&row);
        if seen.insert(key) {
            queue.push(row);
            added += 1;
        } else {
            skipped += 1;
        }
    }

    AppendOutcome { added, skipped }
}

#[cfg(test)]
#[path = "queue/tests.rs"]
mod tests;
