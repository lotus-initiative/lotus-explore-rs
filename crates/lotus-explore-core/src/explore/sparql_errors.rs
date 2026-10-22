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
#[path = "sparql_errors/tests.rs"]
mod tests;
