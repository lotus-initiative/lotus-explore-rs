// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Where an export is downloaded from.
//!
//! Two URLs per format, and they answer different questions. The `QLever` one
//! asks the endpoint to run the query and hand back the file; the `/v1` one asks
//! this app, which is what makes a download work when `QLever` is unreachable
//! and lets the export be rate-limited and logged like any other request.

use lotus_query::ExportFormat;

use lotus_search::QLEVER_WIKIDATA;

/// The `QLever` URL for a query in a format.
///
/// The query is prepared first, so an RDF export is wrapped in the `CONSTRUCT`
/// that makes it triples rather than rows.
#[must_use]
pub fn qlever_export_url(query: &str, format: ExportFormat) -> String {
    format!(
        "{QLEVER_WIKIDATA}?query={}&action={}",
        urlencoding::encode(&format.prepared_query(query)),
        format.qlever_action()
    )
}

/// The app's own URL for a cached export.
#[must_use]
pub fn api_export_file_url(cache_key: &str, format: ExportFormat) -> String {
    format!("/v1/export-file/{cache_key}/{}", format.extension())
}

#[cfg(test)]
#[path = "urls/tests.rs"]
mod tests;
