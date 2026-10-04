// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::features::explore::command::SearchCommand;
use crate::features::explore::error_recovery_coordinator::should_clear_state_on_error;
use crate::features::explore::types::{DomainError, QueryPhase};

use super::super::SearchLifecycleState;

pub(super) fn search_requested(state: &mut SearchLifecycleState, command: SearchCommand) {
    state.loading = true;
    state.error = None;
    state.query_phase = QueryPhase::PreparingQuery;
    state.searched_once = true;
    state.download_only_mode = command.direct_download();
    state.download_dispatching = false;
    state.search_request_token = state.search_request_token.saturating_add(1);
    state.rows_so_far = None;
}

pub(super) const fn phase_changed(state: &mut SearchLifecycleState, phase: QueryPhase) {
    state.query_phase = phase;
}

/// Record progress, and never let it go backwards or outlive the search.
///
/// Both guards are about the same thing: the number on screen must not be one the
/// reader can catch decreasing. A stale chunk arriving after the set is committed
/// would do that, and so would a retry restarting the fetch.
pub(super) fn progress_reported(state: &mut SearchLifecycleState, rows: usize) {
    if !state.loading {
        return;
    }
    state.rows_so_far = Some(state.rows_so_far.map_or(rows, |was| was.max(rows)));
}

pub(super) fn search_finished(state: &mut SearchLifecycleState) {
    state.loading = false;
    state.rows_so_far = None;
    state.error = None;
    state.query_phase = QueryPhase::Idle;
    state.download_dispatching = false;
}

pub(super) fn search_failed(state: &mut SearchLifecycleState, error: &DomainError) -> bool {
    state.loading = false;
    state.rows_so_far = None;
    state.query_phase = QueryPhase::Idle;
    state.download_dispatching = false;
    should_clear_state_on_error(error.query_stage())
}

pub(super) fn record_error(state: &mut SearchLifecycleState, error: DomainError) {
    state.error = Some(error);
}

pub(super) fn dismiss_error(state: &mut SearchLifecycleState) {
    state.error = None;
}

pub(super) const fn download_dispatch_started(state: &mut SearchLifecycleState) {
    state.download_dispatching = true;
}

pub(super) const fn download_dispatch_finished(state: &mut SearchLifecycleState) {
    state.download_dispatching = false;
}
