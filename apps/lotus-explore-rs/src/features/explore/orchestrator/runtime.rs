// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
// The search future is not `Send`: `on_phase` is a closure over a Dioxus
// `Signal`, which is a `RefCell`. See the full explanation in
// `features/explore/executor.rs`, which is the same cause reached from here.
#![expect(
    clippy::future_not_send,
    reason = "`on_phase` captures a Dioxus `Signal`, which is a `RefCell` and not `Sync`"
)]

use super::controller::SearchTaskController;
use crate::features::explore::actions::ExploreAction;
use crate::features::explore::command::SearchCommand;
use crate::features::explore::executor::do_search;
use crate::features::explore::lifecycle::{ErrorHandlingOutcome, SearchLifecycleCoordinator};
use crate::features::explore::outcome::SearchOutcome;
use crate::features::explore::request::SearchRequest;
use crate::features::explore::search_state::{ExploreState, dispatch_explore_action};
use crate::features::explore::service::finalize;
use crate::features::explore::types::DomainError;
use crate::repositories::{LotusRepository, reset_wdqs_fallback_flag};
use crate::services::search_telemetry as telemetry;
use dioxus::prelude::*;
use lotus_search::SearchCriteria;
use std::time::Duration;

const MAX_RETRIES: u32 = 3;

/// Validate input, dispatch `SearchRequested`, then spawn `do_search`.
// `task_controller` is a cheap `Rc<RefCell<…>>` handle consumed as part of
// the argument bundle (callers hold it by `Clone` and pass it down, matching
// the responder-store style); `replace_in_flight` needs only `&self` but the
// callee does not outlive the caller's borrow, so by-value is the fit here.
#[allow(clippy::needless_pass_by_value)]
pub fn start_search<R: LotusRepository>(
    criteria: Signal<SearchCriteria>,
    command: SearchCommand,
    explore: Signal<ExploreState>,
    task_controller: SearchTaskController,
    repo: R,
) {
    let request = match prepare_search_request(criteria, command) {
        Ok(request) => request,
        Err(error) => {
            dispatch_explore_action(explore, ExploreAction::SearchFailed { error, query: None });
            return;
        }
    };

    let Some(run_id) = task_controller.try_begin(request.criteria(), request.command()) else {
        telemetry::search_duplicate_suppressed();
        return;
    };
    reset_wdqs_fallback_flag();
    let request = dispatch_search_request(explore, request);
    let coordinator = SearchLifecycleCoordinator::new(explore);
    let completion_controller = task_controller.clone();
    let task = spawn(async move {
        execute_search_with_retries(request, repo, coordinator).await;
        completion_controller.finish(run_id);
    });
    task_controller.replace_in_flight(task);
}

fn prepare_search_request(
    criteria: Signal<SearchCriteria>,
    command: SearchCommand,
) -> Result<SearchRequest, DomainError> {
    let request = SearchRequest::new(criteria.peek().clone(), command);
    validate_search_criteria(request.criteria())?;
    Ok(request)
}

fn dispatch_search_request(explore: Signal<ExploreState>, request: SearchRequest) -> SearchRequest {
    dispatch_explore_action(explore, request.as_action());
    request.with_request_token(explore.peek().lifecycle.search_request_token)
}

async fn execute_search_with_retries<R: LotusRepository>(
    request: SearchRequest,
    repo: R,
    coordinator: SearchLifecycleCoordinator,
) {
    let mut attempt_count = 0u32;
    loop {
        match do_search(&request, repo.clone(), |phase| coordinator.on_phase(phase)).await {
            Ok(outcome) => {
                handle_search_success(&request, outcome, attempt_count, &coordinator);
                break;
            }
            Err(error) => {
                if !handle_search_error(&request, error, attempt_count, &coordinator).await {
                    break;
                }
                attempt_count = attempt_count.saturating_add(1);
            }
        }
    }
}

fn handle_search_success(
    request: &SearchRequest,
    outcome: SearchOutcome,
    attempt_count: u32,
    coordinator: &SearchLifecycleCoordinator,
) {
    if attempt_count > 0 {
        telemetry::search_success_after_retries(attempt_count);
    }
    coordinator.on_success(request, build_search_succeeded_action(request, outcome));
}

async fn handle_search_error(
    request: &SearchRequest,
    error: DomainError,
    attempt_count: u32,
    coordinator: &SearchLifecycleCoordinator,
) -> bool {
    match coordinator.on_error(request, error, attempt_count, MAX_RETRIES) {
        ErrorHandlingOutcome::RetryScheduled { backoff } => {
            delay_for(backoff).await;
            true
        }
        ErrorHandlingOutcome::Finalized => false,
    }
}

fn validate_search_criteria(criteria: &SearchCriteria) -> Result<(), DomainError> {
    crate::features::explore::form_validation::validate_dispatch_criteria(criteria)
        .map_err(DomainError::Validation)
}

fn build_search_succeeded_action(request: &SearchRequest, outcome: SearchOutcome) -> ExploreAction {
    let SearchOutcome {
        set,
        qid,
        warnings,
        query,
        display_capped_rows,
        endpoint,
    } = outcome;

    let meta = finalize::finalize(
        request.criteria(),
        qid.as_deref(),
        &set,
        request.direct_download(),
        endpoint,
    );

    ExploreAction::SearchSucceeded {
        set,
        qid,
        warnings,
        query,
        display_capped_rows,
        query_hash: meta.query_hash,
        result_hash: meta.result_hash,
        metadata_json: meta.metadata_json,
        endpoint,
    }
}

// Async on both targets for signature parity: WASM awaits a JS timer while
// the native branch only discards `backoff` (native sleeps happen upstream,
// in `execute_search_with_retries`).
#[allow(clippy::unused_async)]
async fn delay_for(backoff: Duration) {
    if backoff.is_zero() {
        return;
    }

    #[cfg(target_arch = "wasm32")]
    {
        let millis = u32::try_from(backoff.as_millis()).unwrap_or(u32::MAX);
        gloo_timers::future::TimeoutFuture::new(millis).await;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = backoff;
    }
}

#[cfg(test)]
pub(super) mod test_exports {
    use super::*;

    pub fn build_search_succeeded_action_for_tests(
        request: &SearchRequest,
        outcome: SearchOutcome,
    ) -> ExploreAction {
        build_search_succeeded_action(request, outcome)
    }

    pub fn validate_search_criteria_for_tests(
        criteria: &SearchCriteria,
    ) -> Result<(), DomainError> {
        validate_search_criteria(criteria)
    }
}
