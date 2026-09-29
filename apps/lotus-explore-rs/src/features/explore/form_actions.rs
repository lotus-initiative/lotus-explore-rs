// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Form action dispatch for `SearchCriteria` mutations.

use crate::models::{ElementState, SearchCriteria, SmilesSearchType};

/// Unified action type for all form field updates.
#[derive(Clone, Debug, PartialEq)]
pub enum FormAction {
    // Taxon + Structure
    Taxon(String),
    Smiles(String),
    SmilesSearchType(SmilesSearchType),
    SmilesThreshold(f64),

    // Mass range
    MassMin(f64),
    MassMax(f64),

    // Year range
    YearMin(u16),
    YearMax(u16),

    // Molecular weight formula
    FormulaEnabled(bool),
    FormulaExact(String),
    CMin(u16),
    CMax(u16),
    HMin(u16),
    HMax(u16),
    NMin(u16),
    NMax(u16),
    OMin(u16),
    OMax(u16),
    PMin(u16),
    PMax(u16),
    SMin(u16),
    SMax(u16),

    // Halogen states
    FState(ElementState),
    ClState(ElementState),
    BrState(ElementState),
    IState(ElementState),
}

/// Apply a `FormAction` to a mutable criteria reference.
/// This is the hot-path reducer used by the form context to avoid cloning the
/// entire `SearchCriteria` on each keystroke.
pub fn apply_form_action_mut(criteria: &mut SearchCriteria, action: FormAction) {
    match action {
        FormAction::Taxon(v) => criteria.taxon = v,
        FormAction::Smiles(v) => criteria.smiles = v,
        FormAction::SmilesSearchType(v) => criteria.smiles_search_type = v,
        FormAction::SmilesThreshold(v) => criteria.smiles_threshold = v,
        FormAction::MassMin(v) => criteria.mass_min = v,
        FormAction::MassMax(v) => criteria.mass_max = v,
        FormAction::YearMin(v) => criteria.year_min = v,
        FormAction::YearMax(v) => criteria.year_max = v,
        FormAction::FormulaEnabled(v) => criteria.formula_enabled = v,
        FormAction::FormulaExact(v) => criteria.formula_exact = v,
        FormAction::CMin(v) => criteria.c_min = v,
        FormAction::CMax(v) => criteria.c_max = v,
        FormAction::HMin(v) => criteria.h_min = v,
        FormAction::HMax(v) => criteria.h_max = v,
        FormAction::NMin(v) => criteria.n_min = v,
        FormAction::NMax(v) => criteria.n_max = v,
        FormAction::OMin(v) => criteria.o_min = v,
        FormAction::OMax(v) => criteria.o_max = v,
        FormAction::PMin(v) => criteria.p_min = v,
        FormAction::PMax(v) => criteria.p_max = v,
        FormAction::SMin(v) => criteria.s_min = v,
        FormAction::SMax(v) => criteria.s_max = v,
        FormAction::FState(v) => criteria.f_state = v,
        FormAction::ClState(v) => criteria.cl_state = v,
        FormAction::BrState(v) => criteria.br_state = v,
        FormAction::IState(v) => criteria.i_state = v,
    }
}

/// Apply a `FormAction` to `SearchCriteria`, returning the updated copy.
/// Kept as a pure helper for reducer-style tests and functional call sites.
#[must_use]
#[cfg(test)]
pub fn apply_form_action(mut criteria: SearchCriteria, action: FormAction) -> SearchCriteria {
    apply_form_action_mut(&mut criteria, action);
    criteria
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_action_mutates_taxon_field() {
        let crit = SearchCriteria {
            taxon: "original".to_string(),
            ..SearchCriteria::default()
        };
        let result = apply_form_action(crit, FormAction::Taxon("updated".to_string()));
        assert_eq!(result.taxon, "updated");
    }

    #[test]
    // The action writes the exact literal `100.5` through; equality must hold
    // bit-for-bit to prove the write-through, so exact float comparison is the
    // meaningful assertion here.
    #[allow(clippy::float_cmp)]
    fn form_action_mutates_mass_range() {
        let crit = SearchCriteria::default();
        let result = apply_form_action(crit, FormAction::MassMin(100.5));
        assert_eq!(result.mass_min, 100.5);
    }

    #[test]
    fn form_action_mutates_element_bounds() {
        let crit = SearchCriteria::default();
        let result = apply_form_action(crit, FormAction::CMin(50));
        assert_eq!(result.c_min, 50);
    }

    #[test]
    fn form_action_mutates_halogen_states() {
        let crit = SearchCriteria::default();
        let result = apply_form_action(crit, FormAction::FState(ElementState::Excluded));
        assert_eq!(result.f_state, ElementState::Excluded);
    }

    #[test]
    fn form_action_immutable_applies_to_copy() {
        let original = SearchCriteria::default();
        let _result = apply_form_action(original.clone(), FormAction::Taxon("test".into()));
        // Original unchanged
        assert_eq!(original.taxon, SearchCriteria::default().taxon);
    }

    #[test]
    fn form_action_mut_updates_existing_reference() {
        let mut criteria = SearchCriteria::default();
        apply_form_action_mut(&mut criteria, FormAction::FormulaEnabled(true));
        apply_form_action_mut(&mut criteria, FormAction::FormulaExact("C15H10O5".into()));
        assert!(criteria.formula_enabled);
        assert_eq!(criteria.formula_exact, "C15H10O5");
    }
}
