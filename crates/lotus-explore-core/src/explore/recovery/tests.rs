// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `recovery`, in their own file.

use super::*;
use crate::ports::RepositoryError;

#[test]
fn validation_errors_not_retryable() {
    use crate::explore::error::ValidationFault;
    let err = DomainError::Validation(ValidationFault::EmptyInput);
    assert!(!is_retryable_error(&err));
}

#[test]
fn network_errors_retryable() {
    use crate::explore::error::QueryStage;
    let err = DomainError::Transport {
        stage: QueryStage::ResultsQuery,
        source: RepositoryError::network("timeout"),
    };
    assert!(is_retryable_error(&err));
}

#[test]
fn http_4xx_not_retryable() {
    use crate::explore::error::QueryStage;
    let err = DomainError::Transport {
        stage: QueryStage::ResultsQuery,
        source: RepositoryError::Http {
            status: 400,
            body: "bad request".into(),
        },
    };
    assert!(!is_retryable_error(&err));
}

#[test]
fn http_5xx_retryable() {
    use crate::explore::error::QueryStage;
    let err = DomainError::Transport {
        stage: QueryStage::ResultsQuery,
        source: RepositoryError::Http {
            status: 502,
            body: "bad gateway".into(),
        },
    };
    assert!(is_retryable_error(&err));
}

#[test]
fn parse_errors_not_retryable() {
    use crate::explore::error::ParseFault;
    let err = DomainError::Parse(ParseFault::ResultsCsv {
        details: "invalid csv row".into(),
    });
    assert!(!is_retryable_error(&err));
}

#[test]
fn not_configured_transport_not_retryable() {
    use crate::explore::error::QueryStage;
    let err = DomainError::Transport {
        stage: QueryStage::ResultsQuery,
        source: RepositoryError::NotConfigured,
    };
    assert!(!is_retryable_error(&err));
}

#[test]
fn cache_conflict_parse_errors_are_retryable() {
    let err = DomainError::Transport {
        stage: crate::explore::error::QueryStage::ResultsQuery,
        source: RepositoryError::parse("Trying to insert a cache key which was already present"),
    };
    assert!(is_retryable_error(&err));
}
