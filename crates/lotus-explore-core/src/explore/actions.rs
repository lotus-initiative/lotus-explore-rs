// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Action catalog for the Explore feature reducer.

use crate::explore::command::SearchCommand;
use crate::explore::types::{DomainError, LookupNotice, QueryPhase};
use crate::export::metadata::SparqlEndpoint;
use crate::sort::SortColumn;
use lotus_model::SearchCriteria;
use std::sync::Arc;

/// All state transitions that can occur in the Explore feature.
#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum ExploreAction {
    /// Start a new search lifecycle.
    SearchRequested {
        /// The criteria the search will run with.
        criteria_snapshot: SearchCriteria,
        /// Whether this was an interactive or download-only request.
        command: SearchCommand,
    },

    /// Update the spinner / lifecycle phase.
    SearchPhaseChanged(QueryPhase),

    /// Rows received so far, for the loading overlay.
    ///
    /// Monotonic within one search, and ignored once the search has finished, so
    /// a chunk that lands after the result is committed cannot put a number back
    /// on screen.
    SearchProgress {
        /// Number of rows received so far.
        rows: usize,
    },

    /// Commit a successful search result set.
    SearchSucceeded {
        /// The complete result set.
        set: std::sync::Arc<lotus_model::ColumnarResultSet>,
        /// The QID of the resolved taxon, when one was resolved.
        qid: Option<String>,
        /// Structured notices about how this search went; formatted at render
        /// time. Empty in the common case.
        warnings: Vec<LookupNotice>,
        /// The SPARQL query that produced the result.
        query: String,
        /// Whether the displayed rows are a capped subset of the full result.
        display_capped_rows: bool,
        /// Hash of the executed query.
        query_hash: Arc<str>,
        /// Hash of the returned result set.
        result_hash: Arc<str>,
        /// Export metadata generated for the result.
        metadata_json: Arc<str>,
        /// The endpoint that served the result.
        endpoint: SparqlEndpoint,
    },

    /// Commit a typed search error (i18n-free; formatted at render time).
    SearchFailed {
        /// The error that ended the search.
        error: DomainError,
        /// The query that failed, when it was built.
        query: Option<String>,
    },

    /// Dismiss the current error notice.
    ErrorDismissed,

    /// Start download dispatching.
    DownloadDispatchStarted,
    /// Finish download dispatching.
    DownloadDispatchFinished,

    /// Toggle a results-table sort column.
    SortToggled(SortColumn),

    /// Replace the results table's column filters.
    ///
    /// The whole set rather than one field, because a keystroke in one column
    /// filter is a keystroke in a set of independent inputs and sending a
    /// partial patch would make the other five fields something to reconstruct.
    FiltersChanged(crate::filters::ColumnFilters),

    /// Forget every column filter.
    FiltersCleared,
}
