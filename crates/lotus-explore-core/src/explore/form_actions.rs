// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Form action dispatch for `SearchCriteria` mutations.

use lotus_model::{ElementState, SearchCriteria, SmilesSearchType};

/// Unified action type for all form field updates.
#[derive(Clone, Debug, PartialEq)]
pub enum FormAction {
    // Taxon + Structure
    Taxon(String),
    /// Which nomenclatural relationship the taxon filter follows, and whether
    /// it is followed at all. One variant for all four rather than one each:
    /// they are the same decision, and a fifth relationship should not mean a
    /// fifth line of reducer.
    TaxonNomenclature {
        /// The relationship being toggled.
        relation: &'static lotus_model::taxon_nomenclature::Relation,
        /// Whether to follow it.
        on: bool,
    },
    Smiles(String),
    /// A Wikidata QID or a DOI in the reference field.
    Reference(String),
    SmilesSearchType(SmilesSearchType),
    SimilarityThreshold(f64),

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
        // The four are one field, so the reducer dispatches on the relation
        // rather than naming a field per variant. A new relationship is one
        // variant here, not five lines of assignment.
        FormAction::TaxonNomenclature { relation, on } => {
            criteria.taxon_names.set_for_relation(relation, on);
        }
        FormAction::Smiles(v) => criteria.structure = v,
        FormAction::Reference(v) => criteria.reference = v,
        FormAction::SmilesSearchType(v) => criteria.structure_search = v,
        FormAction::SimilarityThreshold(v) => criteria.structure_threshold = v,
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
#[path = "form_actions/tests.rs"]
mod tests;
