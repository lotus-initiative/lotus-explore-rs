// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `form_actions`, in their own file.

use super::*;

#[test]
fn form_action_mutates_taxon_field() {
    let crit = SearchCriteria {
        taxon: "original".to_string(),
        ..SearchCriteria::up_to_year(crate::clock::current_year())
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
    let crit = SearchCriteria::up_to_year(crate::clock::current_year());
    let result = apply_form_action(crit, FormAction::MassMin(100.5));
    assert_eq!(result.mass_min, 100.5);
}

#[test]
fn form_action_mutates_element_bounds() {
    let crit = SearchCriteria::up_to_year(crate::clock::current_year());
    let result = apply_form_action(crit, FormAction::CMin(50));
    assert_eq!(result.c_min, 50);
}

#[test]
fn form_action_mutates_halogen_states() {
    let crit = SearchCriteria::up_to_year(crate::clock::current_year());
    let result = apply_form_action(crit, FormAction::FState(ElementState::Excluded));
    assert_eq!(result.f_state, ElementState::Excluded);
}

#[test]
fn form_action_immutable_applies_to_copy() {
    let original = SearchCriteria::up_to_year(crate::clock::current_year());
    let _result = apply_form_action(original.clone(), FormAction::Taxon("test".into()));
    // Original unchanged
    assert_eq!(
        original.taxon,
        SearchCriteria::up_to_year(crate::clock::current_year()).taxon
    );
}

#[test]
fn form_action_mut_updates_existing_reference() {
    let mut criteria = SearchCriteria::up_to_year(crate::clock::current_year());
    apply_form_action_mut(&mut criteria, FormAction::FormulaEnabled(true));
    apply_form_action_mut(&mut criteria, FormAction::FormulaExact("C15H10O5".into()));
    assert!(criteria.formula_enabled);
    assert_eq!(criteria.formula_exact, "C15H10O5");
}
