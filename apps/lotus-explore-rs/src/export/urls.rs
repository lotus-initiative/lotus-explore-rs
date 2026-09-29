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
mod tests {
    use super::*;

    #[test]
    fn the_qlever_url_carries_the_query_and_the_action() {
        let url = qlever_export_url("SELECT ?s WHERE { ?s ?p ?o }", ExportFormat::Csv);
        assert!(url.starts_with(QLEVER_WIKIDATA), "{url}");
        assert!(url.contains("action=csv_export"), "{url}");
        assert!(url.contains("query="), "{url}");
    }

    #[test]
    fn the_query_is_percent_encoded() {
        // An unencoded space or brace in a query string is a malformed URL, and
        // a malformed URL fails at the far end with an error nobody can read.
        let url = qlever_export_url("SELECT ?s WHERE { ?s ?p ?o }", ExportFormat::Csv);
        assert!(!url.contains('{'), "{url}");
        assert!(url.contains("%7B") || url.contains("%20"), "{url}");
    }

    #[test]
    fn an_rdf_export_asks_for_triples_not_rows() {
        let select = "SELECT ?s WHERE { ?s ?p ?o }";
        let url = qlever_export_url(select, ExportFormat::Rdf);
        assert!(url.contains("action=turtle_export"), "{url}");
        assert!(url.contains("CONSTRUCT"), "{url}");
    }

    #[test]
    fn the_api_url_is_the_one_the_server_serves() {
        assert_eq!(
            api_export_file_url("abc123", ExportFormat::Json),
            "/v1/export-file/abc123/json"
        );
        assert_eq!(
            api_export_file_url("abc123", ExportFormat::Rdf),
            "/v1/export-file/abc123/rdf"
        );
    }

    #[test]
    fn a_cache_key_cannot_escape_its_path_segment() {
        // The key reaches the server as a path segment; a slash in it would
        // address a different route.
        assert!(
            !api_export_file_url("a/../b", ExportFormat::Csv)
                .matches('/')
                .count()
                > 3,
            "a key with a slash changes the path"
        );
    }
}
