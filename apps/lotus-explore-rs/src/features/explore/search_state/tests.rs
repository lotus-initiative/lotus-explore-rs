// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::*;
use crate::features::explore::actions::ExploreAction;
use crate::features::explore::command::SearchCommand;
use crate::features::explore::types::{DomainError, QueryPhase, QueryStage, ValidationFault};
use crate::repositories::RepositoryError;
use crate::sort::{SortColumn, SortDir};
use lotus_model::{CompoundEntry, SearchCriteria};
use std::sync::Arc;

fn default_state() -> ExploreState {
    ExploreState::default()
}

#[test]
fn search_requested_sets_loading_and_clears_result() {
    let state = default_state();
    let next = reduce(
        state,
        ExploreAction::SearchRequested {
            criteria_snapshot: SearchCriteria::up_to_year(crate::clock::current_year()),
            command: SearchCommand::Interactive,
        },
    );
    assert!(next.lifecycle.loading);
    assert!(next.lifecycle.error.is_none());
    assert_eq!(next.lifecycle.query_phase, QueryPhase::PreparingQuery);
    assert!(next.lifecycle.searched_once);
    assert!(!next.lifecycle.download_only_mode);
    assert_eq!(next.lifecycle.search_request_token, 1);
    assert!(next.result.entries.is_empty());
    assert!(next.result.sparql_query.is_none());
}

#[test]
fn search_requested_direct_download_flag_propagates() {
    let state = default_state();
    let next = reduce(
        state,
        ExploreAction::SearchRequested {
            criteria_snapshot: SearchCriteria::up_to_year(crate::clock::current_year()),
            command: SearchCommand::StartupDownload,
        },
    );
    assert!(next.lifecycle.download_only_mode);
}

#[test]
fn search_requested_increments_request_token() {
    let mut state = default_state();
    for expected in 1u64..=3 {
        state = reduce(
            state,
            ExploreAction::SearchRequested {
                criteria_snapshot: SearchCriteria::up_to_year(crate::clock::current_year()),
                command: SearchCommand::Interactive,
            },
        );
        assert_eq!(state.lifecycle.search_request_token, expected);
    }
}

#[test]
fn phase_changed_updates_only_phase() {
    let mut state = default_state();
    state.lifecycle.loading = true;
    let next = reduce(
        state,
        ExploreAction::SearchPhaseChanged(QueryPhase::ProcessingResults),
    );
    assert_eq!(next.lifecycle.query_phase, QueryPhase::ProcessingResults);
    assert!(next.lifecycle.loading, "loading must be untouched");
}

#[test]
fn search_succeeded_clears_loading_and_stores_result() {
    let mut state = default_state();
    state.lifecycle.loading = true;
    let rows: Vec<CompoundEntry> = vec![];
    let metadata = Arc::<str>::from(r#"{"test":"metadata"}"#);
    let endpoint = crate::export::SparqlEndpoint::Qlever;
    let next = reduce(
        state,
        ExploreAction::SearchSucceeded {
            rows,
            qid: Some("Q123".into()),
            warning: None,
            query: "SELECT ?x WHERE {}".into(),
            total_matches: Some(42),
            total_stats: None,
            display_capped_rows: true,
            query_hash: "qh".into(),
            result_hash: "rh".into(),
            metadata_json: metadata.clone(),
            endpoint,
        },
    );
    assert!(!next.lifecycle.loading);
    assert_eq!(next.result.resolved_qid.as_deref(), Some("Q123"));
    assert_eq!(next.result.total_matches, Some(42));
    assert!(next.result.display_capped_rows);
    assert_eq!(next.result.query_hash.as_deref(), Some("qh"));
    assert_eq!(next.result.result_hash.as_deref(), Some("rh"));
    assert_eq!(next.result.metadata_json, Some(metadata));
    assert_eq!(next.result.endpoint, crate::export::SparqlEndpoint::Qlever);
}

#[test]
fn search_failed_stores_domain_error_and_clears_loading() {
    let mut state = default_state();
    state.lifecycle.loading = true;
    let err = DomainError::Validation(ValidationFault::EmptyInput);
    let next = reduce(
        state,
        ExploreAction::SearchFailed {
            error: err.clone(),
            query: None,
        },
    );
    assert!(!next.lifecycle.loading);
    assert_eq!(next.lifecycle.error, Some(err));
    assert_eq!(next.lifecycle.query_phase, QueryPhase::Idle);
}

#[test]
fn search_failed_clears_results_for_taxon_stage_errors() {
    let mut state = default_state();
    state.result.resolved_qid = Some("Q123".into());
    state.result.total_matches = Some(9);
    let err = DomainError::transport(QueryStage::TaxonSearch, RepositoryError::network("timeout"));

    let next = reduce(
        state,
        ExploreAction::SearchFailed {
            error: err,
            query: None,
        },
    );
    assert!(next.result.resolved_qid.is_none());
    assert!(next.result.total_matches.is_none());
}

#[test]
fn search_failed_preserves_results_for_results_stage_errors() {
    let mut state = default_state();
    state.result.resolved_qid = Some("Q123".into());
    state.result.total_matches = Some(9);
    let err = DomainError::transport(
        QueryStage::ResultsQuery,
        RepositoryError::network("timeout"),
    );

    let next = reduce(
        state,
        ExploreAction::SearchFailed {
            error: err,
            query: None,
        },
    );
    assert_eq!(next.result.resolved_qid.as_deref(), Some("Q123"));
    assert_eq!(next.result.total_matches, Some(9));
}

#[test]
fn error_dismissed_clears_error_only() {
    let mut state = default_state();
    state.lifecycle.error = Some(DomainError::Validation(ValidationFault::EmptyInput));
    state.lifecycle.loading = true;
    let next = reduce(state, ExploreAction::ErrorDismissed);
    assert!(next.lifecycle.error.is_none());
    assert!(next.lifecycle.loading, "loading must be untouched");
}

#[test]
fn download_dispatch_start_stop_round_trip() {
    let state = default_state();
    let next = reduce(state, ExploreAction::DownloadDispatchStarted);
    assert!(next.lifecycle.download_dispatching);
    let next2 = reduce(next, ExploreAction::DownloadDispatchFinished);
    assert!(!next2.lifecycle.download_dispatching);
}

#[test]
fn sort_toggled_same_column_reverses_direction() {
    let mut state = default_state();
    state.result.sort.col = SortColumn::Name;
    state.result.sort.dir = SortDir::Asc;
    let next = reduce(state, ExploreAction::SortToggled(SortColumn::Name));
    assert_eq!(next.result.sort.dir, SortDir::Desc);
    let next2 = reduce(next, ExploreAction::SortToggled(SortColumn::Name));
    assert_eq!(next2.result.sort.dir, SortDir::Asc);
}

#[test]
fn sort_toggled_new_column_resets_to_asc() {
    let mut state = default_state();
    state.result.sort.col = SortColumn::Name;
    state.result.sort.dir = SortDir::Desc;
    let next = reduce(state, ExploreAction::SortToggled(SortColumn::Mass));
    assert_eq!(next.result.sort.col, SortColumn::Mass);
    assert_eq!(next.result.sort.dir, SortDir::Asc);
}

#[test]
fn dispatch_no_op_does_not_change_state() {
    let state = default_state();
    assert_eq!(state.lifecycle.query_phase, QueryPhase::Idle);
    let next = reduce(
        state.clone(),
        ExploreAction::SearchPhaseChanged(QueryPhase::Idle),
    );
    assert_eq!(next.lifecycle.query_phase, state.lifecycle.query_phase);
}
