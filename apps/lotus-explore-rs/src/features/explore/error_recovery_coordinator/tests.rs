// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `error_recovery_coordinator`, in their own file.

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
        source: RepositoryError::parse("Trying to insert a cache key which was already present"),
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
