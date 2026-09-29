// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! SPARQL endpoint error classification and recovery heuristics.

/// Classify a plain SPARQL/QLever error message.
pub fn classify_sparql_error_text(message: &str) -> SparqlErrorClass {
    let normalized = message.trim().to_ascii_lowercase();

    if normalized.contains("cache key") || normalized.contains("already present") {
        return SparqlErrorClass::CacheConflict;
    }
    if normalized.contains("timeout")
        || normalized.contains("too many")
        || normalized.contains("rate limit")
        || normalized.contains("queue")
    {
        return SparqlErrorClass::RateLimit;
    }
    if normalized.contains("syntax")
        || normalized.contains("grammar")
        || normalized.contains("malformed")
        || normalized.contains("invalid sparql query")
        || normalized.contains("mismatched input")
    {
        return SparqlErrorClass::QuerySyntax;
    }
    if normalized.contains("no results") || normalized.contains("query returned no results") {
        return SparqlErrorClass::NoResults;
    }
    SparqlErrorClass::Unknown
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SparqlErrorClass {
    /// Cache key conflict on upstream endpoint (transient, retry-safe).
    CacheConflict,
    /// Rate limit or query queue full (transient, backoff recommended).
    RateLimit,
    /// Query syntax error (permanent, do not retry).
    QuerySyntax,
    /// No results for valid query (not an error, expected).
    NoResults,
    /// Unclassified error.
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn classify_sparql_exception(json: &Value) -> SparqlErrorClass {
        classify_sparql_error_text(json.get("exception").and_then(|e| e.as_str()).unwrap_or(""))
    }

    fn is_retryable(class: SparqlErrorClass) -> bool {
        matches!(
            class,
            SparqlErrorClass::CacheConflict
                | SparqlErrorClass::RateLimit
                | SparqlErrorClass::Unknown
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
}
