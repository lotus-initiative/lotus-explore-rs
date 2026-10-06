// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `lifecycle`, in their own file.

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
