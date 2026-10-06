// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `state`, in their own file.

use crate::features::explore::{FormAction, apply_form_action};
use lotus_search::SearchCriteria;

#[test]
fn apply_form_action_taxon_round_trips() {
    let base = SearchCriteria::up_to_year(crate::clock::current_year());
    let updated = apply_form_action(base, FormAction::Taxon("Rosa".into()));
    assert_eq!(updated.taxon, "Rosa");
}

#[test]
fn apply_form_action_mass_range_round_trips() {
    let base = SearchCriteria::up_to_year(crate::clock::current_year());
    let updated = apply_form_action(base, FormAction::MassMin(50.0));
    assert!((updated.mass_min - 50.0).abs() < 1e-10);
}

#[test]
fn form_action_all_element_bounds_round_trip() {
    use lotus_model::ElementState;

    let base = SearchCriteria::up_to_year(crate::clock::current_year());
    let updated = apply_form_action(base, FormAction::CMin(6));
    assert_eq!(updated.c_min, 6);

    let base = SearchCriteria::up_to_year(crate::clock::current_year());
    let updated = apply_form_action(base, FormAction::HMax(20));
    assert_eq!(updated.h_max, 20);

    let base = SearchCriteria::up_to_year(crate::clock::current_year());
    let updated = apply_form_action(base, FormAction::FState(ElementState::Required));
    assert_eq!(updated.f_state, ElementState::Required);
}

#[test]
fn form_action_formula_enabled_round_trips() {
    let base = SearchCriteria::up_to_year(crate::clock::current_year());
    assert!(!base.formula_enabled);
    let updated = apply_form_action(base, FormAction::FormulaEnabled(true));
    assert!(updated.formula_enabled);
}

#[test]
fn form_action_smiles_search_type_round_trips() {
    use lotus_model::SmilesSearchType;
    let base = SearchCriteria::up_to_year(crate::clock::current_year());
    let updated = apply_form_action(
        base,
        FormAction::SmilesSearchType(SmilesSearchType::Similarity),
    );
    assert_eq!(updated.structure_search, SmilesSearchType::Similarity);
}

#[test]
fn form_action_year_range_round_trips() {
    let base = SearchCriteria::up_to_year(crate::clock::current_year());
    let updated = apply_form_action(base, FormAction::YearMin(1990));
    assert_eq!(updated.year_min, 1990);
    let base2 = SearchCriteria::up_to_year(crate::clock::current_year());
    let updated = apply_form_action(base2, FormAction::YearMax(2025));
    assert_eq!(updated.year_max, 2025);
}
