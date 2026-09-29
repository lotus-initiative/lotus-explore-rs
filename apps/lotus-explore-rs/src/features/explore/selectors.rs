// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! `use_memo`-based derived selectors for [`ExploreState`].

use crate::features::explore::search_state::{
    ExploreState, ResultDataState, SearchLifecycleState, UiChromeState,
};
use crate::models::{DatasetStats, SearchCriteria};
use dioxus::prelude::*;
use std::sync::Arc;

/// Wrapper around `Arc<T>` that compares by pointer identity.
/// This is useful for large immutable payloads (for example `Arc<[CompoundEntry]>`)
/// where deep `PartialEq` checks are unnecessarily expensive.
#[derive(Clone)]
pub struct ArcPtrEq<T: ?Sized>(pub Arc<T>);

impl<T: ?Sized> PartialEq for ArcPtrEq<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// Subscribe to a derived value from [`SearchLifecycleState`].
/// The component using this memo only re-renders when `f` returns a different
/// value, isolating it from result-data and UI-chrome mutations.
pub fn use_lifecycle_selector<T: PartialEq + Clone + 'static>(
    explore: Signal<ExploreState>,
    f: impl Fn(&SearchLifecycleState) -> T + 'static,
) -> Memo<T> {
    use_memo(move || f(&explore.read().lifecycle))
}

/// Subscribe to a derived value from [`ResultDataState`].
/// The component using this memo only re-renders when `f` returns a different
/// value, isolating it from lifecycle and UI-chrome mutations.
pub fn use_result_selector<T: PartialEq + Clone + 'static>(
    explore: Signal<ExploreState>,
    f: impl Fn(&ResultDataState) -> T + 'static,
) -> Memo<T> {
    use_memo(move || f(&explore.read().result))
}

/// Subscribe to an `Arc<T>` derived from [`ResultDataState`] using pointer
/// equality (`Arc::ptr_eq`) instead of deep value equality.
pub fn use_result_arc_selector<T: ?Sized + 'static>(
    explore: Signal<ExploreState>,
    f: impl Fn(&ResultDataState) -> Arc<T> + 'static,
) -> Memo<ArcPtrEq<T>> {
    use_memo(move || ArcPtrEq(f(&explore.read().result)))
}

/// Subscribe to a derived value from [`UiChromeState`].
/// The component using this memo only re-renders when `f` returns a different
/// value, isolating it from lifecycle and result-data mutations.
pub fn use_ui_selector<T: PartialEq + Clone + 'static>(
    explore: Signal<ExploreState>,
    f: impl Fn(&UiChromeState) -> T + 'static,
) -> Memo<T> {
    use_memo(move || f(&explore.read().ui))
}

/// Subscribe to a derived value from [`SearchCriteria`].
/// The component using this memo only re-renders when `f` returns a different
/// value, isolating it from unrelated criteria-field mutations.
pub fn use_criteria_selector<T: PartialEq + Clone + 'static>(
    criteria: Signal<SearchCriteria>,
    f: impl Fn(&SearchCriteria) -> T + 'static,
) -> Memo<T> {
    use_memo(move || f(&criteria.read()))
}

/// Snapshot of commonly-queried explore UI state flags.
/// Used to reduce signal reads and prevent unnecessary component re-renders
/// across results viewport, table, and toolbar sections.
// Each bool is an independent UI flag read by separate components; collapsing
// them into a state machine would couple unrelated rendering concerns.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ExploreUiState {
    pub loading: bool,
    pub has_error: bool,
    pub searched_once: bool,
    pub download_only_mode: bool,
    pub download_dispatching: bool,
    pub has_entries: bool,
    pub has_query: bool,
    pub has_resolved_qid: bool,
}

impl ExploreUiState {
    pub fn from_explore(explore: Signal<ExploreState>) -> Self {
        let explore_read = explore.read();
        Self {
            loading: explore_read.lifecycle.loading,
            has_error: explore_read.lifecycle.error.is_some(),
            searched_once: explore_read.lifecycle.searched_once,
            download_only_mode: explore_read.lifecycle.download_only_mode,
            download_dispatching: explore_read.lifecycle.download_dispatching,
            has_entries: !explore_read.result.entries.is_empty(),
            has_query: explore_read.result.sparql_query.is_some(),
            has_resolved_qid: explore_read.result.resolved_qid.is_some(),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ToolbarResultSnapshot {
    pub sparql_query: Option<Arc<str>>,
    pub metadata_json: Option<Arc<str>>,
    pub query_hash: Option<Arc<str>>,
    pub result_hash: Option<Arc<str>>,
    pub total_stats: Option<DatasetStats>,
    pub total_matches: Option<usize>,
    pub display_capped_rows: bool,
    pub endpoint: crate::export::SparqlEndpoint,
}

pub fn toolbar_snapshot_from_result(result: &ResultDataState) -> ToolbarResultSnapshot {
    ToolbarResultSnapshot {
        sparql_query: result.sparql_query.clone(),
        metadata_json: result.metadata_json.clone(),
        query_hash: result.query_hash.clone(),
        result_hash: result.result_hash.clone(),
        total_stats: result.total_stats.clone(),
        total_matches: result.total_matches,
        display_capped_rows: result.display_capped_rows,
        endpoint: result.endpoint,
    }
}

pub fn use_toolbar_result_snapshot(explore: Signal<ExploreState>) -> Memo<ToolbarResultSnapshot> {
    use_memo(move || toolbar_snapshot_from_result(&explore.read().result))
}

#[derive(Clone, PartialEq, Eq)]
pub struct HeaderMetaSnapshot {
    pub resolved_qid: Option<Arc<str>>,
    pub query_hash: Option<Arc<str>>,
    pub result_hash: Option<Arc<str>>,
    /// The JSON-LD for this result set, if a search has produced one.
    ///
    /// Carried here rather than read separately because it is derived from the
    /// same result: a snapshot that could show a hash but not the markup would
    /// be describing a page state that does not exist.
    pub metadata_json: Option<Arc<str>>,
}

pub fn header_meta_snapshot_from_result(result: &ResultDataState) -> HeaderMetaSnapshot {
    HeaderMetaSnapshot {
        resolved_qid: result.resolved_qid.clone(),
        query_hash: result.query_hash.clone(),
        result_hash: result.result_hash.clone(),
        metadata_json: result.metadata_json.clone(),
    }
}

pub fn use_header_meta_snapshot(explore: Signal<ExploreState>) -> Memo<HeaderMetaSnapshot> {
    use_memo(move || header_meta_snapshot_from_result(&explore.read().result))
}

#[cfg(test)]
mod tests {
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
            has_entries: !explore.result.entries.is_empty(),
            has_query: explore.result.sparql_query.is_some(),
            has_resolved_qid: explore.result.resolved_qid.is_some(),
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
}
