// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Query-parameter encoding for [`SearchCriteria`].

use super::super::QueryParams;
use lotus_model::{ElementState, SearchCriteria, SmilesSearchType};

/// The `(query-parameter, value)` pairs for a criteria' four toggles, in the
/// order the relations are declared.
///
/// The parameter names are the model's own field names, so a rename on either
/// side has to be made on both — see the round-trip test on
/// [`criteria_query_params`].
const fn nomenclature_query_pairs(
    names: lotus_model::TaxonNomenclature,
) -> [(&'static str, bool); 4] {
    [
        ("taxon_accepted_synonyms", names.accepted_synonyms),
        ("taxon_basionyms", names.basionyms),
        ("taxon_protonyms", names.protonyms),
        ("taxon_replacements", names.replacements),
    ]
}

/// Serialise criteria to query parameters, omitting anything left at its
/// default so a shared URL stays short enough to paste into a message.
///
/// The inverse of [`super::parse_criteria_from_params`], and deliberately in the same
/// place. The parameter names used to live on the model's own method while the
/// parsing lived here, which meant a rename to either side silently broke
/// every URL already in a browser's history and in someone's message. There is
/// a round-trip test below to keep that from happening again.
#[must_use]
pub fn criteria_query_params(criteria: &SearchCriteria, year_max: u16) -> QueryParams {
    let mut params = QueryParams::new();
    insert_taxon_params(criteria, &mut params);
    insert_reference_and_structure_params(criteria, &mut params);
    insert_range_params(criteria, year_max, &mut params);
    insert_formula_params(criteria, &mut params);
    params
}

fn insert_taxon_params(criteria: &SearchCriteria, params: &mut QueryParams) {
    let taxon = criteria.taxon.trim();
    if taxon.is_empty() {
        return;
    }
    params.insert("taxon".to_string(), taxon.to_string());
    // Each nomenclatural toggle is written only when it is off. The default
    // is on, and a link that spells out the default is a link that has to
    // be re-read when the default changes. This is the same rule the mass
    // and year ranges follow.
    for (key, on) in nomenclature_query_pairs(criteria.taxon_names) {
        if !on {
            params.insert(key.to_string(), "false".to_string());
        }
    }
}

fn insert_reference_and_structure_params(criteria: &SearchCriteria, params: &mut QueryParams) {
    // Written as typed, not as resolved: a shared link should show and reproduce
    // the DOI a reader passed, and the lookup is cheap and identical either way.
    let reference = criteria.reference.trim();
    if !reference.is_empty() {
        params.insert("reference".to_string(), reference.to_string());
    }

    if !criteria.structure.trim().is_empty() {
        let structure = lotus_model::normalize_structure(&criteria.structure);
        params.insert("structure".to_string(), structure);
        params.insert(
            "structure_search_type".to_string(),
            criteria.structure_search.as_str().to_string(),
        );
        if criteria.structure_search == SmilesSearchType::Similarity {
            params.insert(
                "similarity_threshold".to_string(),
                format!("{:.2}", criteria.structure_threshold),
            );
        }
    }
}

fn insert_range_params(criteria: &SearchCriteria, year_max: u16, params: &mut QueryParams) {
    // A range that covers the whole domain carries no information, so it is
    // left out rather than written out and read back.
    if criteria.has_mass_filter() {
        params.insert("mass_filter".to_string(), "true".to_string());
        params.insert("mass_min".to_string(), format!("{}", criteria.mass_min));
        params.insert("mass_max".to_string(), format!("{}", criteria.mass_max));
    }

    if criteria.has_year_filter(year_max) {
        params.insert("year_filter".to_string(), "true".to_string());
        params.insert("year_start".to_string(), format!("{}", criteria.year_min));
        params.insert("year_end".to_string(), format!("{}", criteria.year_max));
    }
}

fn insert_formula_params(criteria: &SearchCriteria, params: &mut QueryParams) {
    if !criteria.formula_enabled {
        return;
    }
    params.insert("formula_filter".to_string(), "true".to_string());
    let exact = criteria.formula_exact.trim();
    if !exact.is_empty() {
        params.insert("formula_exact".to_string(), exact.to_string());
    }
    for (label, min, max, default_max) in criteria.element_ranges() {
        let key = label.to_ascii_lowercase();
        if min > 0 {
            params.insert(format!("{key}_min"), min.to_string());
        }
        if max < default_max {
            params.insert(format!("{key}_max"), max.to_string());
        }
    }
    for (label, state) in [
        ("f", criteria.f_state),
        ("cl", criteria.cl_state),
        ("br", criteria.br_state),
        ("i", criteria.i_state),
    ] {
        if state != ElementState::Allowed {
            params.insert(format!("{label}_state"), state.as_str().to_string());
        }
    }
}
