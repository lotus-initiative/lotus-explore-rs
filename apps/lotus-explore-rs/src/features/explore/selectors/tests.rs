// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `selectors`, in their own file.

use super::*;
#[test]
fn explore_ui_state_all_false_by_default() {
    let explore = ExploreState::default();
    let ui_state = ExploreUiState {
        loading: explore.lifecycle.loading,
        has_error: explore.lifecycle.error.is_some(),
        searched_once: explore.lifecycle.searched_once,
        download_only_mode: explore.lifecycle.download_only_mode,
        download_dispatching: explore.lifecycle.download_dispatching,
        has_entries: !explore.result.set.is_empty(),
        has_query: explore.result.sparql_query.is_some(),
        has_resolved_qid: explore.result.resolved_qid.is_some(),
        criteria_dirty: false,
    };

    assert!(!ui_state.loading);
    assert!(!ui_state.has_error);
    assert!(!ui_state.searched_once);
    assert!(!ui_state.download_only_mode);
    assert!(!ui_state.download_dispatching);
    assert!(!ui_state.has_entries);
    assert!(!ui_state.has_query);
    assert!(!ui_state.has_resolved_qid);
}

#[test]
fn toolbar_snapshot_copies_result_fields() {
    let result = ResultDataState {
        sparql_query: Some(Arc::from("SELECT * WHERE { ?s ?p ?o }")),
        metadata_json: Some(Arc::from("{\"k\":\"v\"}")),
        query_hash: Some(Arc::from("qh")),
        result_hash: Some(Arc::from("rh")),
        total_matches: Some(12),
        display_capped_rows: true,
        ..ResultDataState::default()
    };

    let snapshot = toolbar_snapshot_from_result(&result);
    assert_eq!(
        snapshot.sparql_query.as_deref(),
        Some("SELECT * WHERE { ?s ?p ?o }")
    );
    assert_eq!(snapshot.metadata_json.as_deref(), Some("{\"k\":\"v\"}"));
    assert_eq!(snapshot.query_hash.as_deref(), Some("qh"));
    assert_eq!(snapshot.result_hash.as_deref(), Some("rh"));
    assert_eq!(snapshot.total_matches, Some(12));
    assert!(snapshot.display_capped_rows);
}
