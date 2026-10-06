// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `stats`, in their own file.

use super::*;
use crate::criteria::SearchCriteria;

#[test]
fn a_default_search_asks_for_that_one_compound() {
    assert_eq!(SmilesSearchType::default(), SmilesSearchType::Exact);
    assert!(
        !SmilesSearchType::Exact.needs_structure_service(),
        "the default must not load a structure index to answer a name"
    );
    assert!(SmilesSearchType::Substructure.needs_structure_service());
    assert!(SmilesSearchType::Similarity.needs_structure_service());

    let criteria = SearchCriteria::up_to_year(2026);
    assert_eq!(criteria.structure_search, SmilesSearchType::Exact);
    assert_eq!(criteria.structure_threshold, 1.0);
}

#[test]
fn search_type_and_element_state_round_trip_through_their_spellings() {
    for v in [SmilesSearchType::Substructure, SmilesSearchType::Similarity] {
        assert_eq!(SmilesSearchType::parse(v.as_str()), v);
    }
    for v in [
        ElementState::Allowed,
        ElementState::Required,
        ElementState::Excluded,
    ] {
        assert_eq!(ElementState::parse(v.as_str()), v);
    }
}

#[test]
fn an_unrecognised_spelling_falls_back_to_the_default() {
    assert_eq!(SmilesSearchType::parse("fuzzy"), SmilesSearchType::Exact);
    assert_eq!(ElementState::parse("maybe"), ElementState::Allowed);
}
