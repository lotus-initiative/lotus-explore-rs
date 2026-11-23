// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `runtime`, in their own file.

use super::*;

pub fn build_search_succeeded_action_for_tests(
    request: &SearchRequest,
    outcome: SearchOutcome,
) -> ExploreAction {
    build_search_succeeded_action(request, outcome)
}

pub fn validate_search_criteria_for_tests(criteria: &SearchCriteria) -> Result<(), DomainError> {
    validate_search_criteria(criteria)
}

pub fn make_run_finish_guard_for_tests(controller: SearchTaskController, run_id: u64) -> impl Drop {
    RunFinishGuard { controller, run_id }
}
