// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

pub mod form_context;

use crate::app_state::AppState;
use crate::features::explore::ExploreState;
use dioxus::prelude::*;

pub use form_context::{FormCriteriaContext, use_form_criteria_context};

// ── App State Context ─────────────────────────────────────────────────────

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

// ── Results Context ───────────────────────────────────────────────────────────

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

// ── Hook Helpers ──────────────────────────────────────────────────────────────

/// Hook to read the root `AppStateContext` from any descendant component.
pub fn use_app_state_context() -> AppStateContext {
    use_context::<AppStateContext>()
}

pub fn use_app_selector<T: PartialEq + Clone + 'static>(
    app_state: Signal<AppState>,
    f: impl Fn(&AppState) -> T + 'static,
) -> Memo<T> {
    use_memo(move || f(&app_state.read()))
}

pub fn use_results_context() -> ResultsContext {
    use_context::<ResultsContext>()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::features::explore::{FormAction, apply_form_action};
    use crate::models::SearchCriteria;

    #[test]
    fn apply_form_action_taxon_round_trips() {
        let base = SearchCriteria::up_to_year(crate::models::current_year());
        let updated = apply_form_action(base, FormAction::Taxon("Rosa".into()));
        assert_eq!(updated.taxon, "Rosa");
    }

    #[test]
    fn apply_form_action_mass_range_round_trips() {
        let base = SearchCriteria::up_to_year(crate::models::current_year());
        let updated = apply_form_action(base, FormAction::MassMin(50.0));
        assert!((updated.mass_min - 50.0).abs() < 1e-10);
    }

    #[test]
    fn form_action_all_element_bounds_round_trip() {
        use crate::models::ElementState;

        let base = SearchCriteria::up_to_year(crate::models::current_year());
        let updated = apply_form_action(base, FormAction::CMin(6));
        assert_eq!(updated.c_min, 6);

        let base = SearchCriteria::up_to_year(crate::models::current_year());
        let updated = apply_form_action(base, FormAction::HMax(20));
        assert_eq!(updated.h_max, 20);

        let base = SearchCriteria::up_to_year(crate::models::current_year());
        let updated = apply_form_action(base, FormAction::FState(ElementState::Required));
        assert_eq!(updated.f_state, ElementState::Required);
    }

    #[test]
    fn form_action_formula_enabled_round_trips() {
        let base = SearchCriteria::up_to_year(crate::models::current_year());
        assert!(!base.formula_enabled);
        let updated = apply_form_action(base, FormAction::FormulaEnabled(true));
        assert!(updated.formula_enabled);
    }

    #[test]
    fn form_action_smiles_search_type_round_trips() {
        use crate::models::SmilesSearchType;
        let base = SearchCriteria::up_to_year(crate::models::current_year());
        let updated = apply_form_action(
            base,
            FormAction::SmilesSearchType(SmilesSearchType::Similarity),
        );
        assert_eq!(updated.structure_search, SmilesSearchType::Similarity);
    }

    #[test]
    fn form_action_year_range_round_trips() {
        let base = SearchCriteria::up_to_year(crate::models::current_year());
        let updated = apply_form_action(base, FormAction::YearMin(1990));
        assert_eq!(updated.year_min, 1990);
        let base2 = SearchCriteria::up_to_year(crate::models::current_year());
        let updated = apply_form_action(base2, FormAction::YearMax(2025));
        assert_eq!(updated.year_max, 2025);
    }
}
