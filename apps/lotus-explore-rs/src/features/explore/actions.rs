// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Action catalog for the Explore feature reducer.

use crate::export::SparqlEndpoint;
use crate::features::explore::command::SearchCommand;
use crate::features::explore::types::{DomainError, QueryPhase, TaxonWarning};
use crate::sort::SortColumn;
use lotus_model::{CompoundEntry, DatasetStats, SearchCriteria};
use std::sync::Arc;

/// All state transitions that can occur in the Explore feature.
#[derive(Clone, PartialEq)]
pub enum ExploreAction {
    /// Start a new search lifecycle.
    SearchRequested {
        criteria_snapshot: SearchCriteria,
        command: SearchCommand,
    },

    /// Update the spinner / lifecycle phase.
    SearchPhaseChanged(QueryPhase),

    /// Commit a successful search result set.
    SearchSucceeded {
        rows: Vec<CompoundEntry>,
        qid: Option<String>,
        /// Structured notices about how this search went; formatted at render
        /// time. Empty in the common case.
        warnings: Vec<TaxonWarning>,
        query: String,
        total_matches: Option<usize>,
        total_stats: Option<DatasetStats>,
        display_capped_rows: bool,
        query_hash: Arc<str>,
        result_hash: Arc<str>,
        metadata_json: Arc<str>,
        endpoint: SparqlEndpoint,
    },

    /// Commit a typed search error (i18n-free; formatted at render time).
    SearchFailed {
        error: DomainError,
        query: Option<String>,
    },

    /// Dismiss the current error notice.
    ErrorDismissed,

    /// Start/stop download dispatching.
    DownloadDispatchStarted,
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
