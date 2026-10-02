// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::features::explore::actions::ExploreAction;
use dioxus::prelude::*;

use super::{ExploreState, reduce_mut};

fn is_noop(current: &ExploreState, action: &ExploreAction) -> bool {
    match action {
        ExploreAction::SearchPhaseChanged(phase) => current.lifecycle.query_phase == *phase,
        ExploreAction::ErrorDismissed => current.lifecycle.error.is_none(),
        ExploreAction::DownloadDispatchStarted => current.lifecycle.download_dispatching,
        ExploreAction::DownloadDispatchFinished => !current.lifecycle.download_dispatching,
        ExploreAction::FiltersChanged(filters) => current.result.filters == *filters,
        ExploreAction::FiltersCleared => !current.result.filters.is_active(),
        ExploreAction::SearchRequested { .. }
        | ExploreAction::SearchSucceeded { .. }
        | ExploreAction::SearchFailed { .. }
        | ExploreAction::SortToggled(_) => false,
    }
}

pub fn dispatch_explore_action(mut state: Signal<ExploreState>, action: ExploreAction) {
    let current = state.peek();
    if is_noop(&current, &action) {
        return;
    }
    drop(current);

    state.with_mut(|current| reduce_mut(current, action));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::explore::types::QueryPhase;

    #[test]
    fn noop_detection_handles_phase_and_error_actions() {
        let mut state = ExploreState::default();
        state.lifecycle.query_phase = QueryPhase::Idle;

        assert!(is_noop(
            &state,
            &ExploreAction::SearchPhaseChanged(QueryPhase::Idle)
        ));
        assert!(is_noop(&state, &ExploreAction::ErrorDismissed));
        assert!(is_noop(&state, &ExploreAction::DownloadDispatchFinished));
    }
}
