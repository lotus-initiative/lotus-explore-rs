// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `retry`, in their own file.

#![allow(clippy::panic)]

use super::*;
use crate::explore::error::{QueryStage, ValidationFault};
use crate::ports::RepositoryError;

#[test]
fn plan_retry_marks_permanent_errors_without_retrying() {
    let error = DomainError::Validation(ValidationFault::TaxonTooLong);
    let plan = plan_retry(&error, 0, 3);
    assert_eq!(plan.eligibility, RetryEligibility::Permanent);
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
