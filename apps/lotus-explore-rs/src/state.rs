// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

pub mod form_context;

use crate::app_state::AppState;
use crate::features::explore::ExploreState;
use dioxus::prelude::*;

pub use form_context::{FormCriteriaContext, use_form_criteria_context};

/// Root context: access to the unified `AppState` (download, metrics, theme).
/// Use this to read or mutate download-orchestration and theme state.
#[derive(Clone, Copy)]
pub struct AppStateContext {
    pub state: Signal<AppState>,
}

impl AppStateContext {
    pub const fn new(state: Signal<AppState>) -> Self {
        Self { state }
    }
}

/// Context for results-area components.
#[derive(Clone, Copy)]
pub struct ResultsContext {
    /// Live explore signal — results, lifecycle, UI chrome.
    pub explore: Signal<ExploreState>,
}

impl ResultsContext {
    pub const fn new(explore: Signal<ExploreState>) -> Self {
        Self { explore }
    }
}

/// Hook to read the root `AppStateContext` from any descendant component.
pub fn use_app_state_context() -> AppStateContext {
    use_context::<AppStateContext>()
}

pub fn use_results_context() -> ResultsContext {
    use_context::<ResultsContext>()
}

#[cfg(test)]
#[path = "state/tests.rs"]
mod tests;
