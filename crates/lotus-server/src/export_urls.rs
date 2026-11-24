// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Download URL builders for `QLever` direct exports and cached `/v1/export-file` routes.
#![expect(
    clippy::redundant_pub_crate,
    reason = "private crate-root module uses pub(crate) because workspace denies unreachable_pub"
)]

use lotus_query::ExportFormat;
use lotus_search::QLEVER_WIKIDATA;

/// The `QLever` URL for a query in a format.
///
/// The query is prepared first, so an RDF export is wrapped in the `CONSTRUCT`
/// that makes it triples rather than rows.
#[must_use]
pub(crate) fn qlever_export_url(query: &str, format: ExportFormat) -> String {
    format!(
        "{QLEVER_WIKIDATA}?query={}&action={}",
        lotus_model::percent_encode(&format.prepared_query(query)),
        format.qlever_action()
    )
}

/// The server's own URL for a cached export.
#[must_use]
pub(crate) fn api_export_file_url(cache_key: &str, format: ExportFormat) -> String {
    format!("/v1/export-file/{cache_key}/{}", format.extension())
}

#[cfg(test)]
#[path = "export_urls/tests.rs"]
mod tests;
