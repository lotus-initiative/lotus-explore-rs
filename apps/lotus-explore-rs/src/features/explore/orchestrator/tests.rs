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
        rows: Vec::new(),
        qid: Some("Q42".to_string()),
        warnings: Vec::new(),
        query: "SELECT * WHERE {}".to_string(),
        total_matches: Some(7),
        total_stats: None,
        display_capped_rows: true,
        endpoint: crate::export::SparqlEndpoint::Qlever,
    };

    let action = build_search_succeeded_action_for_tests(&request, outcome);
    match action {
        ExploreAction::SearchSucceeded {
            total_matches,
            total_stats,
            display_capped_rows,
            ..
        } => {
            assert_eq!(total_matches, Some(7));
            assert!(total_stats.is_some());
            assert!(display_capped_rows);
        }
        _ => panic!("expected SearchSucceeded action"),
    }
}

#[test]
fn validate_search_criteria_rejects_empty_input() {
    let criteria = SearchCriteria {
        taxon: " ".into(),
        structure: "".into(),
        formula_enabled: false,
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };

    let result = validate_search_criteria_for_tests(&criteria);
    assert_eq!(
        result,
        Err(DomainError::Validation(ValidationFault::EmptyInput))
    );
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
