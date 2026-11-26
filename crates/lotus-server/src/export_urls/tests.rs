// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Unit tests for `export_urls`.

use super::*;

#[test]
fn the_qlever_url_carries_the_query_and_the_action() {
    let url = qlever_export_url("SELECT ?s WHERE { ?s ?p ?o }", ExportFormat::Csv);
    assert!(url.starts_with(lotus_model::QLEVER_WIKIDATA), "{url}");
    assert!(url.contains("action=csv_export"), "{url}");
    assert!(url.contains("query="), "{url}");
}

#[test]
fn the_query_is_percent_encoded() {
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
        "/v1/export-file/abc123/ttl"
    );
}

#[test]
fn a_cache_key_cannot_escape_its_path_segment() {
    let url = api_export_file_url("a/../b", ExportFormat::Csv);
    assert_eq!(
        url.matches('/').count(),
        4,
        "a percent-encoded key must stay in its single path segment: {url}"
    );
    assert!(url.contains("%2F"), "slashes must be encoded: {url}");
}
