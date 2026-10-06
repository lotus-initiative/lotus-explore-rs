// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! `use_memo`-based derived selectors for [`ExploreState`].

use crate::features::explore::search_state::{
    ExploreState, ResultDataState, SearchLifecycleState, UiChromeState,
};
use dioxus::prelude::*;
use lotus_model::{DatasetStats, SearchCriteria};
use std::sync::Arc;

/// Wrapper around `Arc<T>` that compares by pointer identity.
/// This is useful for large immutable payloads (for example the result set)
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

/// Snapshot of the explore UI state flags the viewport, table and toolbar all read.
// Each bool is an independent UI flag read by separate components; collapsing
// them into a state machine would couple unrelated rendering concerns.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent UI flags; a state machine would couple unrelated rendering concerns"
)]
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
    /// The search form has been edited since the last search ran.
    ///
    /// Taken from the form rather than read from here, because the form is a
    /// separate signal and nothing in the explore state knows what the user has
    /// typed.
    pub criteria_dirty: bool,
}

impl ExploreUiState {
    /// `criteria_dirty` is the form's, not the explore state's -- see the field.
    pub fn from_explore(explore: Signal<ExploreState>, criteria_dirty: bool) -> Self {
        let explore_read = explore.read();
        Self {
            loading: explore_read.lifecycle.loading,
            has_error: explore_read.lifecycle.error.is_some(),
            searched_once: explore_read.lifecycle.searched_once,
            download_only_mode: explore_read.lifecycle.download_only_mode,
            download_dispatching: explore_read.lifecycle.download_dispatching,
            has_entries: !explore_read.result.set.is_empty(),
            has_query: explore_read.result.sparql_query.is_some(),
            has_resolved_qid: explore_read.result.resolved_qid.is_some(),
            criteria_dirty,
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
#[path = "selectors/tests.rs"]
mod tests;
