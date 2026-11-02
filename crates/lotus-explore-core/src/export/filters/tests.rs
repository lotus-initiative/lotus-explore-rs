// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `export::filters`, in their own file.

use super::*;
use lotus_model::element_max;

const YEAR: u16 = 2026;

#[test]
fn criteria_with_nothing_set_have_an_empty_filter_block() {
    let criteria = SearchCriteria {
        taxon: "*".into(),
        ..SearchCriteria::up_to_year(YEAR)
    };
    assert_eq!(criteria_to_filters_value(&criteria, YEAR), json!({}));
}

#[test]
fn a_similarity_search_records_its_threshold_and_only_the_narrowed_elements() {
    let criteria = SearchCriteria {
        taxon: "*".into(),
        structure: "CCO".into(),
        structure_search: SmilesSearchType::Similarity,
        structure_threshold: 0.8,
        formula_enabled: true,
        formula_exact: " C2H6O ".into(),
        // A raised minimum alone, and a lowered maximum alone, each narrow the
        // range; bounds left at 0 and at the element's maximum do not.
        c_min: 1,
        h_max: element_max::H - 1,
        f_state: ElementState::Required,
        ..SearchCriteria::up_to_year(YEAR)
    };

    assert_eq!(
        criteria_to_filters_value(&criteria, YEAR),
        json!({
            "chemical_structure": {
                "smiles": "CCO",
                "search_type": "similarity",
                "similarity_threshold": 0.8,
            },
            "molecular_formula": {
                "exact_formula": "C2H6O",
                "carbon": { "min": 1, "max": element_max::C },
                "hydrogen": { "min": 0, "max": element_max::H - 1 },
                "halogens": { "fluorine": "required" },
            },
        })
    );
}

#[test]
fn an_exact_search_has_no_threshold_and_an_exact_formula_alone_has_no_halogens() {
    let criteria = SearchCriteria {
        taxon: "*".into(),
        structure: "CCO".into(),
        structure_search: SmilesSearchType::Exact,
        formula_enabled: true,
        formula_exact: "C2H6O".into(),
        ..SearchCriteria::up_to_year(YEAR)
    };

    assert_eq!(
        criteria_to_filters_value(&criteria, YEAR),
        json!({
            "chemical_structure": { "smiles": "CCO", "search_type": "exact" },
            "molecular_formula": { "exact_formula": "C2H6O" },
        })
    );
}

#[test]
fn a_halogen_constraint_alone_is_a_formula_block_without_an_exact_formula() {
    let criteria = SearchCriteria {
        taxon: "*".into(),
        formula_enabled: true,
        br_state: ElementState::Excluded,
        ..SearchCriteria::up_to_year(YEAR)
    };

    assert_eq!(
        criteria_to_filters_value(&criteria, YEAR),
        json!({ "molecular_formula": { "halogens": { "bromine": "excluded" } } })
    );
}
