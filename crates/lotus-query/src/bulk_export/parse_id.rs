// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Wikidata QID extraction, archive pagination, and endpoint constants for bulk export.

use lotus_model::looks_like_inchikey;

/// Wikidata's entity IRI prefix, stripped to recover a bare QID.
pub const WIKIDATA_ENTITY_PREFIX: &str = "http://www.wikidata.org/entity/";

/// Extract a bare QID (`Q153`) from a Wikidata entity IRI.
///
/// Returns the input unchanged when it is not an entity IRI, matching the
/// Python `extract_qid` helper that all three sources replicate.
#[must_use]
pub fn extract_qid(value: &str) -> &str {
    value
        .strip_prefix(WIKIDATA_ENTITY_PREFIX)
        .or_else(|| value.strip_prefix("https://www.wikidata.org/entity/"))
        .unwrap_or(value)
}

/// Page size for stable `QLever` pagination of the archive queries.
pub const ARCHIVE_PAGE_SIZE: usize = 50_000;

/// Append `LIMIT`/`OFFSET` to a base query for one stable page.
#[must_use]
pub fn archive_page(base_query: &str, offset: usize) -> String {
    format!("{base_query}\nLIMIT {ARCHIVE_PAGE_SIZE}\nOFFSET {offset}")
}

/// Whether a value looks like a standard `InChIKey`, used to validate decoded rows.
#[must_use]
pub fn looks_like_archive_inchikey(value: &str) -> bool {
    looks_like_inchikey(value)
}
