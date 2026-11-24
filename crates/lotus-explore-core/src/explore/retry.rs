// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Retryable search orchestration — integration point for error recovery with the base orchestrator.

use crate::explore::error_recovery::{ErrorClass, classify_error_recovery};
use crate::explore::types::DomainError;

/// Normalized retry decision used by lifecycle/telemetry orchestration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryPlan {
    /// The class of error that produced this plan.
    pub error_class: ErrorClass,
    /// Whether and when the search may be retried.
    pub eligibility: RetryEligibility,
}

/// Compute a complete retry plan from a domain error and attempt counts.
#[must_use]
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
        /// How long to wait before retrying, when the policy specifies a delay.
        backoff_ms: Option<u64>,
        /// The attempt number the retry will be.
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
