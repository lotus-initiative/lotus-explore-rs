// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use lotus_search::SearchCriteria;

use super::super::UiChromeState;

pub(super) fn search_requested(state: &mut UiChromeState, criteria_snapshot: SearchCriteria) {
    state.executed_criteria = criteria_snapshot;
}
