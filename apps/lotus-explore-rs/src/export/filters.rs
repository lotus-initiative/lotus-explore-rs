// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::models::{ElementState, SearchCriteria, SmilesSearchType};
use serde_json::{Map, Value, json};

pub fn criteria_to_filters_value(criteria: &SearchCriteria) -> Value {
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

    if criteria.has_year_filter(crate::models::current_year()) {
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

        for (name, min, max, default_max) in [
            (
                "carbon",
                criteria.c_min,
                criteria.c_max,
                crate::models::DEFAULT_C_MAX,
            ),
            (
                "hydrogen",
                criteria.h_min,
                criteria.h_max,
                crate::models::DEFAULT_H_MAX,
            ),
            (
                "nitrogen",
                criteria.n_min,
                criteria.n_max,
                crate::models::DEFAULT_N_MAX,
            ),
            (
                "oxygen",
                criteria.o_min,
                criteria.o_max,
                crate::models::DEFAULT_O_MAX,
            ),
            (
                "phosphorus",
                criteria.p_min,
                criteria.p_max,
                crate::models::DEFAULT_P_MAX,
            ),
            (
                "sulfur",
                criteria.s_min,
                criteria.s_max,
                crate::models::DEFAULT_S_MAX,
            ),
        ] {
            if min > 0 || max < default_max {
                mf.insert(name.into(), json!({ "min": min, "max": max }));
            }
        }

        let mut hal = Map::new();
        for (name, state) in [
            ("fluorine", criteria.f_state),
            ("chlorine", criteria.cl_state),
            ("bromine", criteria.br_state),
            ("iodine", criteria.i_state),
        ] {
            if state != ElementState::Allowed {
                hal.insert(name.into(), Value::String(state.as_str().into()));
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
