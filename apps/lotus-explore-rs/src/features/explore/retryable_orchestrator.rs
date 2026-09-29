// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Retryable search orchestration — integration point for error recovery with the base orchestrator.

#[cfg(test)]
use crate::features::explore::error_recovery_coordinator::should_clear_state_on_error;
use crate::features::explore::error_recovery_coordinator::{ErrorClass, classify_error_recovery};
use crate::features::explore::types::DomainError;
#[cfg(test)]
use std::time::Duration;

/// Utility to compute retry scheduling for a failed search.
/// Returns the backoff duration before retry attempt, or None if the error is permanent.
#[cfg(test)]
pub fn retry_schedule_delay(
    error: &DomainError,
    attempt_count: u32,
    max_retries: u32,
) -> Option<Duration> {
    match plan_retry(error, attempt_count, max_retries).eligibility {
        RetryEligibility::Retryable { backoff_ms, .. } => backoff_ms.map(Duration::from_millis),
        RetryEligibility::Permanent | RetryEligibility::MaxRetriesExceeded => None,
    }
}

/// Determines whether to preserve partial results when a search fails.
/// Returns `true` if state should be cleared (bad error at early stage),
/// `false` if state should be preserved (e.g., we have previous results to show).
#[cfg(test)]
pub fn preserve_results_on_error(error: &DomainError) -> bool {
    !should_clear_state_on_error(error.query_stage())
}

/// Utility to summarize retry eligibility for UI feedback.
#[cfg(test)]
pub fn retry_eligibility_summary(
    error: &DomainError,
    attempt_count: u32,
    max_retries: u32,
) -> RetryEligibility {
    plan_retry(error, attempt_count, max_retries).eligibility
}

/// Normalized retry decision used by lifecycle/telemetry orchestration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryPlan {
    pub error_class: ErrorClass,
    pub eligibility: RetryEligibility,
}

/// Compute a complete retry plan from a domain error and attempt counts.
pub fn plan_retry(error: &DomainError, attempt_count: u32, max_retries: u32) -> RetryPlan {
    let recovery = classify_error_recovery(error, attempt_count);
    // 429s are throttled over a longer window and amplify into a permanent IP
    // block when retried aggressively: each retry re-runs the entire pipeline
    // (taxon resolution + COUNT + display queries). Cap rate-limit retries far
    // below the generic budget so the endpoint is respected instead of hammered.
    let effective_max = if recovery.error_class == ErrorClass::RateLimit {
        1
    } else {
        max_retries
    };
    let eligibility = if attempt_count >= effective_max {
        RetryEligibility::MaxRetriesExceeded
    } else if recovery.should_retry {
        RetryEligibility::Retryable {
            backoff_ms: recovery.backoff_ms,
            next_attempt_number: attempt_count + 1,
        }
    } else {
        RetryEligibility::Permanent
    };

    RetryPlan {
        error_class: recovery.error_class,
        eligibility,
    }
}

/// Summary of whether/how a search error can be retried.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetryEligibility {
    /// Error is transient and can be retried.
    Retryable {
        backoff_ms: Option<u64>,
        next_attempt_number: u32,
    },
    /// Error is permanent and should not be retried.
    Permanent,
    /// Retry limit has been reached.
    MaxRetriesExceeded,
}
#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]

    use super::*;
    use crate::features::explore::types::{QueryStage, ValidationFault};
    use crate::repositories::RepositoryError;

    #[test]
    fn compute_retry_schedule_returns_none_at_max_retries() {
        let error = DomainError::Validation(ValidationFault::EmptyInput);
        let schedule = retry_schedule_delay(&error, 3, 3);
        assert_eq!(schedule, None);
    }

    #[test]
    fn compute_retry_schedule_returns_none_for_permanent_errors() {
        let error = DomainError::Validation(ValidationFault::EmptyInput);
        let schedule = retry_schedule_delay(&error, 0, 10);
        assert_eq!(schedule, None);
    }

    #[test]
    fn compute_retry_schedule_returns_duration_for_transient_errors() {
        let error = DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            source: RepositoryError::network("connection refused"),
        };
        let schedule = retry_schedule_delay(&error, 0, 3);
        assert!(schedule.is_some());
        assert_eq!(schedule, Some(Duration::from_millis(200)));
    }

    #[test]
    fn should_preserve_results_on_results_query_error() {
        let error = DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            source: RepositoryError::network("timeout"),
        };
        assert!(preserve_results_on_error(&error));
    }

    #[test]
    fn should_not_preserve_results_on_taxon_search_error() {
        let error = DomainError::Transport {
            stage: QueryStage::TaxonSearch,
            source: RepositoryError::network("timeout"),
        };
        assert!(!preserve_results_on_error(&error));
    }

    #[test]
    fn retry_eligibility_summary_for_transient_error() {
        let error = DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            source: RepositoryError::network("connection reset"),
        };
        let summary = retry_eligibility_summary(&error, 0, 3);
        match summary {
            RetryEligibility::Retryable {
                backoff_ms,
                next_attempt_number,
            } => {
                assert_eq!(backoff_ms, Some(200));
                assert_eq!(next_attempt_number, 1);
            }
            _ => panic!("expected retryable error"),
        }
    }

    #[test]
    fn retry_eligibility_summary_for_permanent_error() {
        let error = DomainError::Validation(ValidationFault::TaxonTooLong);
        let summary = retry_eligibility_summary(&error, 0, 3);
        assert_eq!(summary, RetryEligibility::Permanent);
    }

    #[test]
    fn retry_eligibility_summary_max_retries_exceeded() {
        let error = DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            source: RepositoryError::network("timeout"),
        };
        let summary = retry_eligibility_summary(&error, 3, 3);
        assert_eq!(summary, RetryEligibility::MaxRetriesExceeded);
    }

    #[test]
    fn plan_retry_keeps_error_class_and_retryability_in_sync() {
        let error = DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            source: RepositoryError::network("timeout"),
        };

        let plan = plan_retry(&error, 0, 3);
        assert_eq!(plan.error_class.as_key(), "network");
        assert_eq!(
            plan.eligibility,
            RetryEligibility::Retryable {
                backoff_ms: Some(200),
                next_attempt_number: 1,
            }
        );
    }

    #[test]
    fn plan_retry_reports_max_retries_without_losing_error_class() {
        let error = DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            source: RepositoryError::network("timeout"),
        };

        let plan = plan_retry(&error, 3, 3);
        assert_eq!(plan.error_class.as_key(), "network");
        assert_eq!(plan.eligibility, RetryEligibility::MaxRetriesExceeded);
    }

    #[test]
    fn plan_retry_caps_rate_limit_retries() {
        let error = DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            source: RepositoryError::parse("Too many queries in queue"),
        };

        // attempt 0: RateLimit is retryable with the long 1s backoff.
        let plan = plan_retry(&error, 0, 3);
        assert!(matches!(
            plan.eligibility,
            RetryEligibility::Retryable {
                backoff_ms: Some(1000),
                next_attempt_number: 1,
            }
        ));

        // attempt 1: capped (effective_max = 1) -> stop hammering Qlever.
        let plan = plan_retry(&error, 1, 3);
        assert_eq!(plan.eligibility, RetryEligibility::MaxRetriesExceeded);
    }
}
