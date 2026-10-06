// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `hybrid`, in their own file.

use super::*;

#[test]
fn strip_limit_removes_trailing_limit() {
    let q = "SELECT ?s WHERE { ?s ?p ?o } LIMIT 500";
    assert_eq!(strip_limit_clause(q), "SELECT ?s WHERE { ?s ?p ?o }");
}

#[test]
fn strip_limit_preserves_query_without_limit() {
    let q = "SELECT ?s WHERE { ?s ?p ?o }";
    assert_eq!(strip_limit_clause(q), "SELECT ?s WHERE { ?s ?p ?o }");
}

#[test]
fn strip_limit_preserves_inner_limit() {
    let q = "SELECT ?s WHERE { { SELECT ?s WHERE { ?s ?p ?o } LIMIT 10 } LIMIT 500";
    // Should only strip the last LIMIT
    assert_eq!(
        strip_limit_clause(q),
        "SELECT ?s WHERE { { SELECT ?s WHERE { ?s ?p ?o } LIMIT 10 }"
    );
}

#[test]
fn strip_limit_handles_case_insensitive() {
    let q = "SELECT ?s WHERE { ?s ?p ?o } limit 500";
    assert_eq!(strip_limit_clause(q), "SELECT ?s WHERE { ?s ?p ?o }");
}

#[test]
fn strip_limit_handles_newline() {
    let q = "SELECT ?s WHERE { ?s ?p ?o }\nLIMIT 500";
    assert_eq!(strip_limit_clause(q), "SELECT ?s WHERE { ?s ?p ?o }");
}

#[test]
fn map_fetch_error_preserves_http_status_and_body() {
    let mapped = map_fetch_error(FetchError::Http {
        status: 400,
        message: "invalid query".to_string(),
    });
    assert_eq!(
        mapped,
        RepositoryError::Http {
            status: 400,
            body: "invalid query".to_string(),
        }
    );
}

#[test]
fn map_fetch_error_keeps_network_as_network() {
    let mapped = map_fetch_error(FetchError::Network("timeout".to_string()));
    assert!(matches!(mapped, RepositoryError::Network(_)));
}

#[test]
fn qlever_unavailability_includes_502_and_network_failures() {
    let gateway = FetchError::Http {
        status: 502,
        message: "upstream gateway error (HTML payload)".into(),
    };
    assert!(is_qlever_unavailable(&gateway));
    assert!(is_qlever_unavailable(&FetchError::Network(
        "connection failed".into()
    )));
    assert!(!is_qlever_unavailable(&FetchError::Http {
        status: 500,
        message: "boom".into()
    }));
    assert!(!is_qlever_unavailable(&FetchError::Http {
        status: 400,
        message: "bad query".into()
    }));
    assert!(!is_qlever_unavailable(&FetchError::Empty));
}
