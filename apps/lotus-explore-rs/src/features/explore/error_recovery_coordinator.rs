// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Error recovery coordination — intelligent retry decisions based on error classification.

use crate::features::explore::transport_classification::{
    TransportFailureKind, classify_transport_error,
};
use crate::features::explore::types::{DomainError, QueryStage};
use crate::repositories::RepositoryError;

/// Encapsulates retry decision-making for a failed search operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryDecision {
    /// Whether the search should be retried.
    pub should_retry: bool,
    /// If retrying, how long to wait (milliseconds) before attempting.
    pub backoff_ms: Option<u64>,
    /// Short classification of the error for telemetry/logging.
    pub error_class: ErrorClass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorClass {
    /// Validation error (user input is wrong, don't retry).
    Validation,
    /// Environment/application configuration problem.
    Configuration,
    /// Network timeout or connection error (transient, retry with backoff).
    Network,
    /// HTTP 5xx or comparable upstream failure.
    Server,
    /// Upstream SPARQL cache conflict (transient, retry immediately).
    CacheConflict,
    /// Rate limit or query queue full (transient, retry with backoff).
    RateLimit,
    /// The endpoint cancelled the query for running over its time budget.
    ///
    /// Not transient, and the distinction from `RateLimit` is the point of the
    /// class: `RateLimit` means the endpoint wants fewer requests per second,
    /// this means it wants a smaller query.
    QueryTooExpensive,
    /// HTTP 4xx request rejected for non-syntax reasons.
    BadRequest,
    /// Query syntax error (permanent, don't retry).
    QuerySyntax,
    /// Parse error (permanent, don't retry).
    Parse,
    /// The result set was cut short (permanent for this query, don't retry:
    /// a retry re-downloads the same prefix and arrives at the same place).
    Truncated,
    /// Memory pressure in the browser runtime.
    #[cfg(target_arch = "wasm32")]
    Memory,
}

impl ErrorClass {
    #[must_use]
    pub const fn as_key(self) -> &'static str {
        match self {
            Self::Validation => "validation",
            Self::Configuration => "configuration",
            Self::Network => "network",
            Self::Server => "server",
            Self::CacheConflict => "cache_conflict",
            Self::RateLimit => "rate_limit",
            Self::QueryTooExpensive => "query_too_expensive",
            Self::BadRequest => "bad_request",
            Self::QuerySyntax => "query_syntax",
            Self::Parse => "parse",
            Self::Truncated => "truncated",
            #[cfg(target_arch = "wasm32")]
            Self::Memory => "memory",
        }
    }
}

/// Determine retry strategy for a failed search given the error and attempt count.
pub fn classify_error_recovery(error: &DomainError, attempt: u32) -> RetryDecision {
    match error {
        DomainError::Validation(_) => RetryDecision {
            should_retry: false,
            backoff_ms: None,
            error_class: ErrorClass::Validation,
        },

        DomainError::Parse(_) => RetryDecision {
            should_retry: false,
            backoff_ms: None,
            error_class: ErrorClass::Parse,
        },

        DomainError::Transport { source, .. } => classify_transport_error_recovery(source, attempt),

        #[cfg(target_arch = "wasm32")]
        DomainError::MemoryLimit { .. } => RetryDecision {
            should_retry: false,
            backoff_ms: None,
            error_class: ErrorClass::Memory,
        },
    }
}

/// Classify a transport-layer error and determine retry strategy.
fn classify_transport_error_recovery(repo_error: &RepositoryError, attempt: u32) -> RetryDecision {
    match classify_transport_error(repo_error) {
        TransportFailureKind::Configuration => RetryDecision {
            should_retry: false,
            backoff_ms: None,
            error_class: ErrorClass::Configuration,
        },
        TransportFailureKind::Network => RetryDecision {
            should_retry: true,
            backoff_ms: Some(backoff_delay_ms(attempt)),
            error_class: ErrorClass::Network,
        },

        TransportFailureKind::Server => RetryDecision {
            should_retry: true,
            backoff_ms: Some(backoff_delay_ms(attempt)),
            error_class: ErrorClass::Server,
        },

        TransportFailureKind::CacheConflict => RetryDecision {
            should_retry: true,
            backoff_ms: Some(100),
            error_class: ErrorClass::CacheConflict,
        },

        TransportFailureKind::RateLimit => RetryDecision {
            should_retry: true,
            // Qlever throttles over a short window and is pushed into a
            // *permanent* IP block when hammered with the default 100 ms base
            // backoff (which retries the whole pipeline ~4x in ~1.4s). Use a
            // longer, capped backoff so the endpoint's window can reset between
            // retries, and cap the number of retries in `plan_retry`.
            backoff_ms: Some(rate_limit_backoff_ms(attempt)),
            error_class: ErrorClass::RateLimit,
        },

        // No retry, because this is not a busy endpoint. QLever cancels a query
        // that outran its budget and answers 429; the pipeline below would re-run
        // taxon resolution, the lookups and the results query for the same
        // cancellation at the same cost. Only a narrower query changes the
        // answer, and that is the reader's decision.
        TransportFailureKind::QueryTooExpensive => RetryDecision {
            should_retry: false,
            backoff_ms: None,
            error_class: ErrorClass::QueryTooExpensive,
        },

        TransportFailureKind::BadRequest => RetryDecision {
            should_retry: false,
            backoff_ms: None,
            error_class: ErrorClass::BadRequest,
        },

        TransportFailureKind::QuerySyntax => RetryDecision {
            should_retry: false,
            backoff_ms: None,
            error_class: ErrorClass::QuerySyntax,
        },

        TransportFailureKind::Parse => RetryDecision {
            should_retry: false,
            backoff_ms: None,
            error_class: ErrorClass::Parse,
        },

        // No retry, and the reason is the whole point of the variant: the
        // pipeline that produced this had already transferred most of a very
        // large body by the time it stopped, and retrying restarts from byte
        // zero. Three attempts is not resilience here, it is three times the
        // wait the user already sat through.
        TransportFailureKind::Truncated => RetryDecision {
            should_retry: false,
            backoff_ms: None,
            error_class: ErrorClass::Truncated,
        },
    }
}

/// Compute exponential backoff for retry attempt.
/// Base 100ms, doubles each attempt (`2^(attempt+1)`), capped at 10s.
fn backoff_delay_ms(attempt: u32) -> u64 {
    const BASE_MS: u64 = 100;
    const MAX_MS: u64 = 10_000;
    let exponent = (attempt + 1).min(7); // 100 * 2^7 = 12_800, rounded down to cap
    (BASE_MS << exponent).min(MAX_MS)
}

/// Backoff for 429 / rate-limit retries.
fn rate_limit_backoff_ms(attempt: u32) -> u64 {
    const BASE_MS: u64 = 1_000;
    const MAX_MS: u64 = 10_000;
    // 1 000 * 2^attempt, capped at 10s.
    let exponent = attempt.min(4);
    (BASE_MS << exponent).min(MAX_MS)
}

/// Determine whether partial results should be cleared after error at given stage.
pub const fn should_clear_state_on_error(error_stage: QueryStage) -> bool {
    match error_stage {
        QueryStage::TaxonSearch => true, // Everything downstream is invalid
        QueryStage::ResultsQuery => false, // Keep previous results visible
    }
}

#[cfg(test)]
#[path = "error_recovery_coordinator/tests.rs"]
mod tests;
