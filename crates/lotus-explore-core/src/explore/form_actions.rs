// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Form action dispatch for `SearchCriteria` mutations.

use lotus_model::{ElementState, SearchCriteria, SmilesSearchType};

/// Unified action type for all form field updates.
#[derive(Clone, Debug, PartialEq)]
pub enum FormAction {
    /// Replace the taxon field.
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
    /// Replace the structure field.
    Smiles(String),
    /// A Wikidata QID or a DOI in the reference field.
    Reference(String),
    /// Change how the structure field is searched.
    SmilesSearchType(SmilesSearchType),
    /// Change the similarity threshold for a structure search.
    SimilarityThreshold(f64),

    // Mass range
    /// Set the minimum molecular mass.
    MassMin(f64),
    /// Set the maximum molecular mass.
    MassMax(f64),

    // Year range
    /// Set the earliest publication year.
    YearMin(u16),
    /// Set the latest publication year.
    YearMax(u16),

    // Molecular weight formula
    /// Enable or disable the molecular-formula filter.
    FormulaEnabled(bool),
    /// Set the exact molecular formula.
    FormulaExact(String),
    /// Set the minimum number of carbon atoms.
    CMin(u16),
    /// Set the maximum number of carbon atoms.
    CMax(u16),
    /// Set the minimum number of hydrogen atoms.
    HMin(u16),
    /// Set the maximum number of hydrogen atoms.
    HMax(u16),
    /// Set the minimum number of nitrogen atoms.
    NMin(u16),
    /// Set the maximum number of nitrogen atoms.
    NMax(u16),
    /// Set the minimum number of oxygen atoms.
    OMin(u16),
    /// Set the maximum number of oxygen atoms.
    OMax(u16),
    /// Set the minimum number of phosphorus atoms.
    PMin(u16),
    /// Set the maximum number of phosphorus atoms.
    PMax(u16),
    /// Set the minimum number of sulfur atoms.
    SMin(u16),
    /// Set the maximum number of sulfur atoms.
    SMax(u16),

    // Halogen states
    /// Set the presence filter for fluorine.
    FState(ElementState),
    /// Set the presence filter for chlorine.
    ClState(ElementState),
    /// Set the presence filter for bromine.
    BrState(ElementState),
    /// Set the presence filter for iodine.
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
pub fn apply_form_action(mut criteria: SearchCriteria, action: FormAction) -> SearchCriteria {
    apply_form_action_mut(&mut criteria, action);
    criteria
}

#[cfg(test)]
#[path = "form_actions/tests.rs"]
mod tests;
