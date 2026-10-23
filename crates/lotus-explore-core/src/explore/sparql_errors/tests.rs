// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `sparql_errors`, in their own file.

use super::*;
use serde_json::{Value, json};

fn classify_sparql_exception(json: &Value) -> SparqlErrorClass {
    classify_sparql_error_text(json.get("exception").and_then(|e| e.as_str()).unwrap_or(""))
}

fn is_retryable(class: SparqlErrorClass) -> bool {
    matches!(
        class,
        SparqlErrorClass::CacheConflict | SparqlErrorClass::RateLimit | SparqlErrorClass::Unknown
    )
}

fn should_backoff(class: SparqlErrorClass) -> bool {
    matches!(class, SparqlErrorClass::RateLimit)
}

#[test]
fn classify_sparql_exception_maps_known_error_shapes() {
    assert_eq!(
        classify_sparql_exception(&json!({ "exception": "cache key conflict" })),
        SparqlErrorClass::CacheConflict
    );
    assert_eq!(
        classify_sparql_exception(&json!({ "exception": "timeout while running query" })),
        SparqlErrorClass::RateLimit
    );
    assert_eq!(
        classify_sparql_exception(&json!({ "exception": "syntax error near SELECT" })),
        SparqlErrorClass::QuerySyntax
    );
    assert_eq!(
        classify_sparql_exception(&json!({ "exception": "no results" })),
        SparqlErrorClass::NoResults
    );
    assert_eq!(
        classify_sparql_exception(&json!({ "exception": "unexpected upstream failure" })),
        SparqlErrorClass::Unknown
    );
}

#[test]
fn classify_sparql_error_text_detects_qlever_parser_messages() {
    assert_eq!(
        classify_sparql_error_text(
            "Invalid SPARQL query: Token \"AS\": mismatched input 'AS' expecting ','"
        ),
        SparqlErrorClass::QuerySyntax
    );
}

#[test]
fn retryability_helpers_match_classification_contract() {
    assert!(is_retryable(SparqlErrorClass::CacheConflict));
    assert!(is_retryable(SparqlErrorClass::RateLimit));
    assert!(is_retryable(SparqlErrorClass::Unknown));
    assert!(!is_retryable(SparqlErrorClass::QuerySyntax));
    assert!(!is_retryable(SparqlErrorClass::NoResults));
    assert!(should_backoff(SparqlErrorClass::RateLimit));
    assert!(!should_backoff(SparqlErrorClass::CacheConflict));
}
