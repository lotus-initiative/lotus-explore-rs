// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::explore::sparql_errors::{SparqlErrorClass, classify_sparql_error_text};
use crate::ports::RepositoryError;

/// Normalized transport-failure semantics shared by retry policy, UI hinting,
/// and lifecycle telemetry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportFailureKind {
    /// The endpoint or repository has not been configured.
    Configuration,
    /// The network could not reach the endpoint.
    Network,
    /// The endpoint returned a server-side failure.
    Server,
    /// The request was rejected as invalid.
    BadRequest,
    /// The endpoint rejected a conflicting cached query.
    CacheConflict,
    /// The endpoint refused the request because of rate limiting.
    RateLimit,
    /// The endpoint cancelled the query because it outran its time budget.
    ///
    /// Its own kind, not a flavour of `RateLimit`, because the two call for
    /// opposite behaviour: a rate limit asks the caller to come back, this
    /// measures the query as too expensive and is answered by asking for less.
    /// Retrying it spends the endpoint's whole budget again to be told the same
    /// thing.
    QueryTooExpensive,
    /// The SPARQL query is syntactically invalid.
    QuerySyntax,
    /// A response could not be parsed.
    Parse,
    /// The result set stopped short of the whole answer.
    Truncated,
}

impl TransportFailureKind {
    /// Whether retrying the same request could plausibly succeed.
    #[must_use]
    pub const fn is_retryable(self) -> bool {
        matches!(
            self,
            Self::Network | Self::Server | Self::CacheConflict | Self::RateLimit
        )
    }
}

/// Classify a repository error into the transport semantics shared by the app.
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
        return if mentions_timeout(body) && !says_slow_down(body) {
            TransportFailureKind::QueryTooExpensive
        } else {
            TransportFailureKind::RateLimit
        };
    }

    if (status == 504 || (status >= 500 && mentions_timeout(body))) && !says_slow_down(body) {
        return TransportFailureKind::QueryTooExpensive;
    }

    match classify_sparql_error_text(body) {
        SparqlErrorClass::CacheConflict => TransportFailureKind::CacheConflict,
        // A client error is a rate limit only when it says so. "timeout" or
        // "queue" in a 4xx is a request the endpoint refuses outright -- `QLever`'s
        // 403 for a time budget above its ceiling -- and resending it gets the
        // same refusal.
        SparqlErrorClass::RateLimit if (400..500).contains(&status) && !says_slow_down(body) => {
            TransportFailureKind::BadRequest
        }
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

/// Whether a body asks the client to send less, rather than to send differently.
fn says_slow_down(body: &str) -> bool {
    let body = body.to_ascii_lowercase();
    body.contains("too many") || body.contains("rate limit")
}

/// Whether a body indicates an upstream query execution timeout.
fn mentions_timeout(body: &str) -> bool {
    let body = body.to_ascii_lowercase();
    body.contains("timed out") || body.contains("time limit") || body.contains("timeout")
}

fn classify_parse_error(detail: &str) -> TransportFailureKind {
    if super::truncation::truncation_reason(detail).is_some() {
        return TransportFailureKind::Truncated;
    }
    match classify_sparql_error_text(detail) {
        SparqlErrorClass::CacheConflict => TransportFailureKind::CacheConflict,
        SparqlErrorClass::RateLimit if mentions_timeout(detail) && !says_slow_down(detail) => {
            TransportFailureKind::QueryTooExpensive
        }
        SparqlErrorClass::RateLimit => TransportFailureKind::RateLimit,
        SparqlErrorClass::QuerySyntax => TransportFailureKind::QuerySyntax,
        SparqlErrorClass::NoResults | SparqlErrorClass::Unknown => TransportFailureKind::Parse,
    }
}

#[cfg(test)]
#[path = "transport_classification/tests.rs"]
mod tests;
