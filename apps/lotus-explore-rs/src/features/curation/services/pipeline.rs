// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
// No `future_not_send` suppression needed: `curate_rows` takes a callback
// returning a concrete `Fut`, and the future it builds is `Send`. Other curation
// modules hold a Dioxus `Signal` across their awaits and do need one; this one
// passes a closure touching nothing thread-local.

use crate::i18n::Locale;
use lotus_curation::{
    CurationError, CurationInputRow, CurationResultRow, QuickStatementsBundle,
    build_quickstatements_bundle,
};
use std::future::Future;

// Curation drives Qlever with several POSTs per row (compound fetch, ASK, ...).
// QLever's anonymous quota rejects bursts, so rows are curated one at a time.

pub async fn curate_rows<F, Fut>(
    locale: Locale,
    rows: Vec<CurationInputRow>,
    curate_single_row: F,
    row_uniqueness_key: fn(&CurationInputRow) -> String,
) -> Result<(Vec<CurationResultRow>, QuickStatementsBundle), CurationError>
where
    F: Fn(Locale, CurationInputRow) -> Fut,
    Fut: Future<Output = CurationResultRow>,
{
    let mut seen_keys = std::collections::HashSet::with_capacity(rows.len());
    let mut unique_rows = Vec::with_capacity(rows.len());
    for row in rows {
        if seen_keys.insert(row_uniqueness_key(&row)) {
            unique_rows.push(row);
        }
    }

    let mut results = Vec::with_capacity(unique_rows.len());
    for row in unique_rows {
        results.push(curate_single_row(locale, row).await);
    }
    let bundle = build_quickstatements_bundle(&results);
    Ok((results, bundle))
}

#[cfg(test)]
#[path = "pipeline/tests.rs"]
mod tests;
