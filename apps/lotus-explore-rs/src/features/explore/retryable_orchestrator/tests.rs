// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `retryable_orchestrator`, in their own file.

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
