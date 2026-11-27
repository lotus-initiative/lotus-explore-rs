// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use lotus_model::{ElementState, SearchCriteria, SmilesSearchType};
use serde_json::{Map, Value, json};

/// The search's criteria as the JSON filter block the metadata document
/// carries.
///
/// `year` is the current year, because whether the year bounds mean anything
/// is a question about today rather than about the search.
#[must_use]
pub(in crate::export) fn criteria_to_filters_value(criteria: &SearchCriteria, year: u16) -> Value {
    let mut filters = Map::new();

    if !criteria.structure.trim().is_empty() {
        let mut cs = Map::new();
        cs.insert("smiles".into(), Value::String(criteria.structure.clone()));
        cs.insert(
            "search_type".into(),
            Value::String(criteria.structure_search.as_str().into()),
        );
        if criteria.structure_search == SmilesSearchType::Similarity {
            cs.insert(
                "similarity_threshold".into(),
                json!(criteria.structure_threshold),
            );
        }
        filters.insert("chemical_structure".into(), Value::Object(cs));
    }

    if criteria.has_mass_filter() {
        filters.insert(
            "mass".into(),
            json!({ "min": criteria.mass_min, "max": criteria.mass_max }),
        );
    }

    if criteria.has_year_filter(year) {
        filters.insert(
            "publication_year".into(),
            json!({ "start": criteria.year_min, "end": criteria.year_max }),
        );
    }

    if criteria.has_formula_filter() {
        let mut mf = Map::new();
        let exact = criteria.formula_exact.trim();
        if !exact.is_empty() {
            mf.insert("exact_formula".into(), Value::String(exact.into()));
        }

        for (symbol, min, max, default_max) in criteria.element_ranges() {
            if min > 0 || max < default_max {
                mf.insert(
                    element_full_name(symbol).into(),
                    json!({ "min": min, "max": max }),
                );
            }
        }

        let mut hal = Map::new();
        for (symbol, state) in criteria.halogen_states() {
            if state != ElementState::Allowed {
                hal.insert(
                    element_full_name(symbol).into(),
                    Value::String(state.as_str().into()),
                );
            }
        }
        if !hal.is_empty() {
            mf.insert("halogens".into(), Value::Object(hal));
        }

        if !mf.is_empty() {
            filters.insert("molecular_formula".into(), Value::Object(mf));
        }
    }

    Value::Object(filters)
}

fn element_full_name(symbol: &str) -> &'static str {
    match symbol {
        "C" => "carbon",
        "H" => "hydrogen",
        "N" => "nitrogen",
        "O" => "oxygen",
        "P" => "phosphorus",
        "S" => "sulfur",
        "F" => "fluorine",
        "Cl" => "chlorine",
        "Br" => "bromine",
        "I" => "iodine",
        _ => "",
    }
}

#[cfg(test)]
#[path = "filters/tests.rs"]
mod tests;
