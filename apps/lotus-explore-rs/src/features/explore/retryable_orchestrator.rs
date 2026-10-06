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
#[path = "retryable_orchestrator/tests.rs"]
mod tests;
