// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `config`, in their own file.

use super::*;

#[test]
fn normalize_base_trims_trailing_slash() {
    assert_eq!(
        normalize_api_base("https://api.example.org/"),
        Some("https://api.example.org".to_string())
    );
}

#[test]
fn normalize_base_rejects_non_http_scheme() {
    assert_eq!(normalize_api_base("ftp://api.example.org"), None);
    assert_eq!(normalize_api_base("api.example.org"), None);
}

#[test]
fn normalize_base_rejects_sparql_endpoints() {
    assert_eq!(normalize_api_base("https://api/wikidata"), None);
    assert_eq!(normalize_api_base("https://qlever.dev/api/wikidata"), None);
    assert_eq!(
        normalize_api_base("https://query.wikidata.org/sparql"),
        None
    );
}
