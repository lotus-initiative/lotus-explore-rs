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
    /// Its own kind, not a flavour of `RateLimit`, because the two call for
    /// opposite behaviour: a rate limit asks the caller to come back, this
    /// measures the query as too expensive and is answered by asking for less.
    /// Retrying it spends the endpoint's whole budget again to be told the same
    /// thing.
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
#[path = "transport_classification/tests.rs"]
mod tests;
