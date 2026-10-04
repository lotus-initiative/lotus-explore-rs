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
        return TransportFailureKind::RateLimit;
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
    fn configuration_and_network_have_expected_retryability() {
        assert!(!TransportFailureKind::Configuration.is_retryable());
        assert!(TransportFailureKind::Network.is_retryable());
    }
}
