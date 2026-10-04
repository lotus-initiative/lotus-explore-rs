// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::features::explore::sparql_errors::{SparqlErrorClass, classify_sparql_error_text};
use crate::repositories::RepositoryError;

/// Normalized transport-failure semantics shared by retry policy, UI hinting,
/// and lifecycle telemetry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportFailureKind {
    Configuration,
    Network,
    Server,
    BadRequest,
    CacheConflict,
    RateLimit,
    /// The endpoint cancelled the query because it outran its time budget.
    ///
    /// Its own kind, and not a flavour of `RateLimit`, because the two call for
    /// opposite behaviour. A rate limit is a request to come back; this is a
    /// measurement that the query is too expensive, and it is answered by asking
    /// for less. Retrying it, which `RateLimit` still allows once, spends the
    /// endpoint's whole budget again to be told the same thing.
    QueryTooExpensive,
    QuerySyntax,
    Parse,
    Truncated,
}

impl TransportFailureKind {
    #[must_use]
    pub const fn is_retryable(self) -> bool {
        matches!(
            self,
            Self::Network | Self::Server | Self::CacheConflict | Self::RateLimit
        )
    }
}

#[must_use]
pub fn classify_transport_error(error: &RepositoryError) -> TransportFailureKind {
    match error {
        RepositoryError::NotConfigured => TransportFailureKind::Configuration,
        RepositoryError::Network(_) => TransportFailureKind::Network,
        RepositoryError::Http { status, body } => classify_http_error(*status, body),
        RepositoryError::Parse(detail) => classify_parse_error(detail.as_ref()),
        // Deliberately not in `is_retryable`. Repeating a wide query to get the
        // same truncation re-transfers however many hundreds of megabytes
        // already arrived, which is the slowness rather than the cure.
        RepositoryError::Truncated(_) => TransportFailureKind::Truncated,
    }
}

fn classify_http_error(status: u16, body: &str) -> TransportFailureKind {
    if status == 429 {
        // QLever uses 429 for two unrelated things, and the body is the only
        // thing that tells them apart: "Operation timed out" is the query
        // outstaying its budget, while "too many" is a request-rate refusal.
        // Both are 429, and treating the first as the second is what produced
        // four full-length queries for one search.
        return if body.contains("timed out") {
            TransportFailureKind::QueryTooExpensive
        } else {
            TransportFailureKind::RateLimit
        };
    }

    match classify_sparql_error_text(body) {
        SparqlErrorClass::CacheConflict => TransportFailureKind::CacheConflict,
        SparqlErrorClass::RateLimit => TransportFailureKind::RateLimit,
        SparqlErrorClass::QuerySyntax => TransportFailureKind::QuerySyntax,
        SparqlErrorClass::NoResults | SparqlErrorClass::Unknown => {
            if (400..500).contains(&status) {
                TransportFailureKind::BadRequest
            } else {
                TransportFailureKind::Server
            }
        }
    }
}

fn classify_parse_error(detail: &str) -> TransportFailureKind {
    match classify_sparql_error_text(detail) {
        SparqlErrorClass::CacheConflict => TransportFailureKind::CacheConflict,
        SparqlErrorClass::RateLimit => TransportFailureKind::RateLimit,
        SparqlErrorClass::QuerySyntax => TransportFailureKind::QuerySyntax,
        SparqlErrorClass::NoResults | SparqlErrorClass::Unknown => TransportFailureKind::Parse,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_http_syntax_error_as_query_syntax() {
        let kind = classify_transport_error(&RepositoryError::Http {
            status: 400,
            body: "Invalid SPARQL query: Token \"AS\": mismatched input 'AS' expecting ','"
                .to_string(),
        });
        assert_eq!(kind, TransportFailureKind::QuerySyntax);
    }

    #[test]
    fn classifies_http_5xx_without_known_signature_as_server() {
        let kind = classify_transport_error(&RepositoryError::Http {
            status: 503,
            body: "service unavailable".to_string(),
        });
        assert_eq!(kind, TransportFailureKind::Server);
    }

    #[test]
    fn classifies_parse_cache_conflict_as_cache_conflict() {
        let kind = classify_transport_error(&RepositoryError::parse(
            "Trying to insert a cache key which was already present",
        ));
        assert_eq!(kind, TransportFailureKind::CacheConflict);
    }

    #[test]
    fn a_429_that_says_the_query_timed_out_is_not_a_rate_limit() {
        // Both arrive as 429 and they mean opposite things. QLever's own answer,
        // measured:
        //
        //   429 {"exception":"Operation timed out. Last operation: Sort ... on ?r"}
        //
        // is one query outstaying the endpoint's budget. Treating it as a busy
        // endpoint is what made one broad search cost four full-length queries.
        let cancelled = classify_transport_error(&RepositoryError::Http {
            status: 429,
            body: "Operation timed out. Last operation: Sort (internal order) on ?r".to_string(),
        });
        assert_eq!(cancelled, TransportFailureKind::QueryTooExpensive);
        assert!(
            !cancelled.is_retryable(),
            "the same query sent again will be cancelled the same way"
        );

        // A proxy in front of the endpoint can still answer 429 for a request
        // rate, and that one is worth coming back to.
        let refused = classify_transport_error(&RepositoryError::Http {
            status: 429,
            body: "Too many requests from this client".to_string(),
        });
        assert_eq!(refused, TransportFailureKind::RateLimit);
    }

    #[test]
    fn configuration_and_network_have_expected_retryability() {
        assert!(!TransportFailureKind::Configuration.is_retryable());
        assert!(TransportFailureKind::Network.is_retryable());
    }
}
