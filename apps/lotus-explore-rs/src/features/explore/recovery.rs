// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Error recovery strategies and stale state handling patterns.

use crate::features::explore::transport_classification::classify_transport_error;
use crate::features::explore::types::DomainError;
use crate::repositories::RepositoryError;

/// Determines whether an error is recoverable and worth retrying.
#[must_use]
pub fn is_retryable_error(error: &DomainError) -> bool {
    match error {
        // Validation errors should NEVER retry — user input is wrong.
        DomainError::Validation(_) => false,

        // Network errors MAY be transient — connection issues, timeouts, 502s.
        DomainError::Transport { source, .. } => is_retryable_transport_error(source),

        // Parse errors indicate data corruption or format change — don't retry.
        DomainError::Parse(_) => false,

        // Memory limits are usually permanent in WASM — don't retry.
        #[cfg(target_arch = "wasm32")]
        DomainError::MemoryLimit { .. } => false,
    }
}

/// Determines whether a repository/network error is transient and worth retrying.
#[must_use]
pub fn is_retryable_transport_error(error: &RepositoryError) -> bool {
    classify_transport_error(error).is_retryable()
}

/// Determine whether a "Retry" button should be shown for this error.
#[must_use]
pub fn should_show_retry_button(error: &DomainError) -> bool {
    is_retryable_error(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_errors_not_retryable() {
        use crate::features::explore::types::ValidationFault;
        let err = DomainError::Validation(ValidationFault::EmptyInput);
        assert!(!is_retryable_error(&err));
    }

    #[test]
    fn network_errors_retryable() {
        use crate::features::explore::types::QueryStage;
        let err = DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            source: RepositoryError::network("timeout"),
        };
        assert!(is_retryable_error(&err));
    }

    #[test]
    fn http_4xx_not_retryable() {
        use crate::features::explore::types::QueryStage;
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
        use crate::features::explore::types::QueryStage;
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
        use crate::features::explore::types::ParseFault;
        let err = DomainError::Parse(ParseFault::ResultsCsv {
            details: "invalid csv row".into(),
        });
        assert!(!is_retryable_error(&err));
    }

    #[test]
    fn not_configured_transport_not_retryable() {
        use crate::features::explore::types::QueryStage;
        let err = DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            source: RepositoryError::NotConfigured,
        };
        assert!(!is_retryable_error(&err));
    }

    #[test]
    fn retry_button_shown_only_for_retryable_errors() {
        use crate::features::explore::types::{QueryStage, ValidationFault};
        let validation_err = DomainError::Validation(ValidationFault::EmptyInput);
        assert!(!should_show_retry_button(&validation_err));

        let network_err = DomainError::Transport {
            stage: QueryStage::ResultsQuery,
            source: RepositoryError::network("timeout"),
        };
        assert!(should_show_retry_button(&network_err));
    }

    #[test]
    fn cache_conflict_parse_errors_are_retryable() {
        let err = DomainError::Transport {
            stage: crate::features::explore::types::QueryStage::ResultsQuery,
            source: RepositoryError::parse(
                "Trying to insert a cache key which was already present",
            ),
        };
        assert!(is_retryable_error(&err));
    }
}
