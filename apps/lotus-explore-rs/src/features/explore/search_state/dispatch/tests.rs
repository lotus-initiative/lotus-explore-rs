// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `dispatch`, in their own file.

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
