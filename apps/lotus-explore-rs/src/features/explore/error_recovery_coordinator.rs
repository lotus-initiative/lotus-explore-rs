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
mod tests {
    use super::*;
    use crate::features::explore::types::ValidationFault;

    #[test]
    fn validation_error_does_not_retry() {
        let err = DomainError::Validation(ValidationFault::EmptyInput);
        let decision = classify_error_recovery(&err, 0);
        assert!(!decision.should_retry);
        assert_eq!(decision.error_class, ErrorClass::Validation);
    }

    #[test]
    fn network_error_retries_with_backoff() {
        let err = DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            source: RepositoryError::network("connection refused"),
        };
        let decision = classify_error_recovery(&err, 0);
        assert!(decision.should_retry);
        assert_eq!(decision.backoff_ms, Some(200)); // 100 * 2^1
        assert_eq!(decision.error_class, ErrorClass::Network);
    }

    #[test]
    fn cache_conflict_retries_immediately() {
        let err = DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            source: RepositoryError::parse(
                "Trying to insert a cache key which was already present",
            ),
        };
        let decision = classify_error_recovery(&err, 0);
        assert!(decision.should_retry);
        assert_eq!(decision.backoff_ms, Some(100)); // Immediate retry
        assert_eq!(decision.error_class, ErrorClass::CacheConflict);
    }

    #[test]
    fn rate_limit_error_retries_with_backoff() {
        let err = DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            source: RepositoryError::parse("Too many queries in queue"),
        };
        // Rate-limit backoff starts at 1s (not 400ms) to let Qlever's window reset.
        let decision = classify_error_recovery(&err, 1);
        assert!(decision.should_retry);
        assert_eq!(decision.backoff_ms, Some(2000)); // 1_000 * 2^1
        assert_eq!(decision.error_class, ErrorClass::RateLimit);
    }

    #[test]
    fn rate_limit_backoff_grows_longer_capped() {
        assert_eq!(rate_limit_backoff_ms(0), 1_000);
        assert_eq!(rate_limit_backoff_ms(1), 2_000);
        assert_eq!(rate_limit_backoff_ms(2), 4_000);
        assert_eq!(rate_limit_backoff_ms(6), 10_000); // capped at 10s
    }

    #[test]
    fn a_cut_short_result_set_is_not_retried() {
        // The distinction the whole `Truncated` variant exists to draw. A network
        // error is retried; this is a network error that arrived after most of a
        // very large body had been transferred, and retrying it means
        // re-transferring all of that to be cut off in the same place. Three
        // attempts is not resilience, it is three times the wait.
        let err = DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            source: RepositoryError::truncated(
                "the result set was cut short after 512000000 bytes: \
                 no data arrived within the timeout",
            ),
        };
        let decision = classify_error_recovery(&err, 0);
        assert!(!decision.should_retry);
        assert_eq!(decision.backoff_ms, None);
        assert_eq!(decision.error_class, ErrorClass::Truncated);
    }

    #[test]
    fn a_cut_short_result_set_is_not_retried_however_many_attempts_have_gone() {
        // `attempt` is the retry counter, and a truncation that survives to a
        // later attempt must still not be retried: it is not a failure that is
        // getting better.
        let err = DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            source: RepositoryError::truncated("cut short"),
        };
        for attempt in 0..4 {
            assert!(
                !classify_error_recovery(&err, attempt).should_retry,
                "attempt {attempt} retried a truncated result set"
            );
        }
    }

    #[test]
    fn syntax_error_is_permanent() {
        let err = DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            source: RepositoryError::parse("SPARQL syntax error near WHERE"),
        };
        let decision = classify_error_recovery(&err, 0);
        assert!(!decision.should_retry);
        assert_eq!(decision.error_class, ErrorClass::QuerySyntax);
    }

    #[test]
    fn http_syntax_error_is_classified_without_retry() {
        let err = DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            source: RepositoryError::Http {
                status: 400,
                body: "Invalid SPARQL query: mismatched input 'AS' expecting ','".into(),
            },
        };
        let decision = classify_error_recovery(&err, 0);
        assert!(!decision.should_retry);
        assert_eq!(decision.error_class, ErrorClass::QuerySyntax);
    }

    #[test]
    fn http_5xx_is_retryable_server_error() {
        let err = DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            source: RepositoryError::Http {
                status: 503,
                body: "temporary upstream failure".into(),
            },
        };
        let decision = classify_error_recovery(&err, 0);
        assert!(decision.should_retry);
        assert_eq!(decision.error_class, ErrorClass::Server);
        assert_eq!(decision.backoff_ms, Some(200));
    }

    #[test]
    fn not_configured_is_a_configuration_error() {
        let err = DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            source: RepositoryError::NotConfigured,
        };
        let decision = classify_error_recovery(&err, 0);
        assert!(!decision.should_retry);
        assert_eq!(decision.error_class, ErrorClass::Configuration);
    }

    #[test]
    fn backoff_strategy_grows_exponentially_capped() {
        assert_eq!(backoff_delay_ms(0), 200);
        assert_eq!(backoff_delay_ms(1), 400);
        assert_eq!(backoff_delay_ms(2), 800);
        assert_eq!(backoff_delay_ms(6), 10_000); // Capped at 10s (100 * 2^7 would be 12_800)
        assert_eq!(backoff_delay_ms(7), 10_000); // Still capped
        assert_eq!(backoff_delay_ms(10), 10_000); // Still capped
    }

    #[test]
    fn should_clear_state_differs_by_stage() {
        assert!(should_clear_state_on_error(QueryStage::TaxonSearch));
        assert!(!should_clear_state_on_error(QueryStage::ResultsQuery));
    }
}
