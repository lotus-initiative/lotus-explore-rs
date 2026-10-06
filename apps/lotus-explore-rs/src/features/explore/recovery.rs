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
#[path = "recovery/tests.rs"]
mod tests;
