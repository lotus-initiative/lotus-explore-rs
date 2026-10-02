// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Lifecycle coordination for explore search execution.

use crate::features::explore::actions::ExploreAction;
use crate::features::explore::request::SearchRequest;
use crate::features::explore::retryable_orchestrator::{RetryEligibility, plan_retry};
use crate::features::explore::search_state::{ExploreState, dispatch_explore_action};
use crate::features::explore::types::{DomainError, QueryPhase};
use crate::services::search_telemetry as telemetry;
use dioxus::prelude::*;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorHandlingOutcome {
    RetryScheduled { backoff: Duration },
    Finalized,
}

#[derive(Clone, Copy)]
pub struct SearchLifecycleCoordinator {
    explore: Signal<ExploreState>,
}

impl SearchLifecycleCoordinator {
    #[must_use]
    pub const fn new(explore: Signal<ExploreState>) -> Self {
        Self { explore }
    }

    pub fn on_phase(&self, phase: QueryPhase) {
        dispatch_explore_action(self.explore, ExploreAction::SearchPhaseChanged(phase));
    }

    pub fn on_success(&self, request: &SearchRequest, success_action: ExploreAction) {
        if is_stale_token(request.request_token(), self.current_token()) {
            telemetry::ignored_stale_result(request.request_token());
            return;
        }

        for action in success_transition_actions(success_action) {
            dispatch_explore_action(self.explore, action);
        }
    }

    pub fn on_error(
        &self,
        request: &SearchRequest,
        error: DomainError,
        attempt_count: u32,
        max_retries: u32,
    ) -> ErrorHandlingOutcome {
        if is_stale_token(request.request_token(), self.current_token()) {
            telemetry::ignored_stale_error(request.request_token());
            return ErrorHandlingOutcome::Finalized;
        }

        let retry_plan = plan_retry(&error, attempt_count, max_retries);
        telemetry::search_error_classified(
            retry_plan.error_class.as_key(),
            attempt_count,
            matches!(retry_plan.eligibility, RetryEligibility::Retryable { .. }),
        );

        match retry_plan.eligibility {
            RetryEligibility::Retryable {
                backoff_ms,
                next_attempt_number,
            } => {
                let backoff = Duration::from_millis(backoff_ms.unwrap_or(0));
                telemetry::search_retry_scheduled(
                    retry_plan.error_class.as_key(),
                    next_attempt_number,
                    u64::try_from(backoff.as_millis()).unwrap_or(u64::MAX),
                );
                ErrorHandlingOutcome::RetryScheduled { backoff }
            }
            RetryEligibility::MaxRetriesExceeded => {
                telemetry::search_max_retries_exceeded(
                    retry_plan.error_class.as_key(),
                    attempt_count,
                );
                dispatch_error(self.explore, error, request);
                ErrorHandlingOutcome::Finalized
            }
            RetryEligibility::Permanent => {
                dispatch_error(self.explore, error, request);
                ErrorHandlingOutcome::Finalized
            }
        }
    }

    fn current_token(&self) -> u64 {
        self.explore.peek().lifecycle.search_request_token
    }
}

fn dispatch_error(explore: Signal<ExploreState>, error: DomainError, request: &SearchRequest) {
    use crate::features::explore::service::build_query::{
        ResolvedStructure, build_sparql_query, normalize_smiles,
    };

    // Try to build the query that was being attempted. It is rebuilt from the
    // criteria alone, so a search that resolved a name shows the unresolved
    // shape -- which is the best that can be done without re-resolving, and is
    // enough to point a reader at what was attempted.
    let target = ResolvedStructure::unresolved(&normalize_smiles(&request.criteria().structure));
    let query = explore
        .peek()
        .result
        .resolved_qid
        .as_deref()
        .map(|qid| build_sparql_query(&target, request.criteria(), Some(qid)))
        .or_else(|| Some(build_sparql_query(&target, request.criteria(), None)));

    dispatch_explore_action(explore, ExploreAction::SearchFailed { error, query });
}

#[must_use]
pub const fn is_stale_token(request_token: u64, current_token: u64) -> bool {
    request_token != current_token
}

#[must_use]
pub const fn success_transition_actions(success_action: ExploreAction) -> [ExploreAction; 2] {
    [
        ExploreAction::SearchPhaseChanged(QueryPhase::Rendering),
        success_action,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_token_detection_requires_exact_match() {
        assert!(!is_stale_token(4, 4));
        assert!(is_stale_token(4, 5));
    }

    #[test]
    fn success_actions_emit_rendering_before_result_commit() {
        let actions = success_transition_actions(ExploreAction::ErrorDismissed);
        assert!(matches!(
            actions[0],
            ExploreAction::SearchPhaseChanged(QueryPhase::Rendering)
        ));
        assert!(matches!(actions[1], ExploreAction::ErrorDismissed));
    }
}
