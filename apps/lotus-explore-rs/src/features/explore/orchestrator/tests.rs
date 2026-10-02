// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

#![allow(clippy::manual_string_new)]
#![allow(clippy::panic)]

use super::runtime::test_exports::{
    build_search_succeeded_action_for_tests, validate_search_criteria_for_tests,
};
use crate::features::explore::actions::ExploreAction;
use crate::features::explore::command::SearchCommand;
use crate::features::explore::outcome::SearchOutcome;
use crate::features::explore::request::SearchRequest;
use crate::features::explore::types::{DomainError, ValidationFault};
use lotus_search::SearchCriteria;

#[test]
fn build_search_succeeded_action_applies_finalized_counts() {
    let request = SearchRequest::new(
        SearchCriteria::up_to_year(crate::clock::current_year()),
        SearchCommand::Interactive,
    );
    let outcome = SearchOutcome {
        set: std::sync::Arc::new(lotus_model::ColumnarResultSet::default()),
        qid: Some("Q42".to_string()),
        warnings: Vec::new(),
        query: "SELECT * WHERE {}".to_string(),
        display_capped_rows: true,
        endpoint: crate::export::SparqlEndpoint::Qlever,
    };

    let action = build_search_succeeded_action_for_tests(&request, outcome);
    match action {
        ExploreAction::SearchSucceeded {
            set,
            display_capped_rows,
            ..
        } => {
            // The counts are no longer carried on the action: the set holds them,
            // so there is no field that could disagree with the rows on screen.
            assert_eq!(set.row_count(), 0);
            assert_eq!(set.stats().n_entries, 0);
            assert!(display_capped_rows);
        }
        _ => panic!("expected SearchSucceeded action"),
    }
}

#[test]
fn validate_search_criteria_accepts_a_search_that_names_nothing() {
    // The orchestrator no longer refuses this. A search with no structure and no
    // taxon is answered, with a notice saying it covers all of LOTUS -- which is
    // what makes the answer legible rather than surprising.
    let criteria = SearchCriteria {
        taxon: " ".into(),
        structure: "".into(),
        formula_enabled: false,
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };

    let result = validate_search_criteria_for_tests(&criteria);
    assert_eq!(result, Ok(()));
}

#[test]
fn validate_search_criteria_accepts_formula_only_input() {
    let criteria = SearchCriteria {
        taxon: "".into(),
        structure: "".into(),
        formula_enabled: true,
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };

    assert_eq!(validate_search_criteria_for_tests(&criteria), Ok(()));
}

#[test]
fn validate_search_criteria_maps_shared_mass_validation_fault() {
    let criteria = SearchCriteria {
        taxon: "Rosa".into(),
        mass_min: -1.0,
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };

    assert_eq!(
        validate_search_criteria_for_tests(&criteria),
        Err(DomainError::Validation(ValidationFault::MassOutOfRange))
    );
}
