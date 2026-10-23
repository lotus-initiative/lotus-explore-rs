// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
use crate::explore::types::{DomainError, LookupNotice, QueryPhase};
use crate::filters::ColumnFilters;
use crate::sort::SortState;
use lotus_model::{ColumnarResultSet, DatasetStats, SearchCriteria};
use std::sync::Arc;

/// Lifecycle-related fields: loading flag, current error, phase indicator,
/// and bookkeeping tokens. Changes here should re-render loading overlays
/// and error notices.
// The booleans are independent UI flags consumed by separate selectors/
// components (loading overlay, download toolbar, error strip); packing them
// into a state-machine enum would couple unrelated rendering concerns.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent UI flags; a state machine would couple unrelated rendering concerns"
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchLifecycleState {
    /// Whether a search is currently running.
    pub loading: bool,
    /// The last error, if any.
    pub error: Option<DomainError>,
    /// The phase the current (or last) search is in.
    pub query_phase: QueryPhase,
    /// Whether a search has ever completed.
    pub searched_once: bool,
    /// Whether the current search runs only to prepare a download.
    pub download_only_mode: bool,
    /// Whether a download is currently being dispatched.
    pub download_dispatching: bool,
    /// Monotonic token identifying the active search request.
    pub search_request_token: u64,
    /// Rows folded into the set so far, while a search is still running.
    ///
    /// `None` outside a fetch. There is deliberately no percentage and no
    /// expected total: the endpoint does not say how many rows a query will
    /// produce before it produces them, so any denominator would be invented.
    pub rows_so_far: Option<usize>,
}

impl Default for SearchLifecycleState {
    fn default() -> Self {
        Self {
            loading: false,
            error: None,
            query_phase: QueryPhase::Idle,
            searched_once: false,
            download_only_mode: false,
            download_dispatching: false,
            search_request_token: 0,
            rows_so_far: None,
        }
    }
}

/// Result payload and presentation state. Changes here re-render the results
/// table, toolbar, header-meta row, and taxon notice.
#[derive(Clone, Debug, PartialEq)]
pub struct ResultDataState {
    /// The whole result set, stored by column.
    ///
    /// Every row the endpoint returned: there is no row limit on this path, and
    /// no separate count query, because the set computes its own counts. Rows are
    /// only turned into [`CompoundEntry`](lotus_model::CompoundEntry) values for
    /// the handful on screen.
    pub set: Arc<ColumnarResultSet>,
    /// Everything worth telling the user about how the taxon resolved, rendered
    /// as one notice line each.
    pub lookup_notices: Vec<LookupNotice>,
    /// The QID of the taxon that was resolved, when one was.
    pub resolved_qid: Option<Arc<str>>,
    /// Hash of the executed query, when available.
    pub query_hash: Option<Arc<str>>,
    /// Hash of the returned result set, when available.
    pub result_hash: Option<Arc<str>>,
    /// The SPARQL query that produced this result, when available.
    pub sparql_query: Option<Arc<str>>,
    /// Export metadata generated for this result, when available.
    pub metadata_json: Option<Arc<str>>,
    /// Total number of matching records reported by the endpoint.
    pub total_matches: Option<usize>,
    /// Dataset statistics gathered while fetching.
    pub total_stats: Option<DatasetStats>,
    /// Whether the displayed rows are a capped subset of the full result.
    pub display_capped_rows: bool,
    /// The current sort order of the results table.
    pub sort: SortState,
    /// Client-side narrowing of the rows already fetched.
    ///
    /// Part of the result state rather than of the form criteria on purpose:
    /// these never reach the SPARQL query, they are not in the URL, and a new
    /// search clears them along with the rows they were filtering. See
    /// [`crate::filters`].
    pub filters: ColumnFilters,
    /// The SPARQL endpoint used (Qlever by default, WDQS on fallback from 502)
    pub endpoint: crate::export::metadata::SparqlEndpoint,
}

impl Default for ResultDataState {
    fn default() -> Self {
        Self {
            set: Arc::new(ColumnarResultSet::default()),
            lookup_notices: Vec::new(),
            resolved_qid: None,
            query_hash: None,
            result_hash: None,
            sparql_query: None,
            metadata_json: None,
            total_matches: None,
            total_stats: None,
            display_capped_rows: false,
            sort: SortState::default(),
            filters: ColumnFilters::empty(),
            endpoint: crate::export::metadata::SparqlEndpoint::Qlever,
        }
    }
}

/// UI chrome and the last-executed criteria snapshot. Changes here re-render
/// the query toolbar.
#[derive(Clone, Debug, PartialEq)]
pub struct UiChromeState {
    /// The criteria of the search whose results are on screen.
    pub executed_criteria: SearchCriteria,
}

impl UiChromeState {
    /// Create a new UI chrome state with the given year.
    #[must_use]
    pub const fn new(year: u16) -> Self {
        Self {
            executed_criteria: SearchCriteria::up_to_year(year),
        }
    }
}

impl Default for UiChromeState {
    fn default() -> Self {
        Self::new(2026)
    }
}

impl ExploreState {
    /// Create a new explore state with the given year.
    #[must_use]
    pub fn new(year: u16) -> Self {
        Self {
            lifecycle: SearchLifecycleState::default(),
            result: ResultDataState::default(),
            ui: UiChromeState::new(year),
        }
    }
}

/// The complete Explore feature state: lifecycle, results, and UI chrome.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ExploreState {
    /// Loading, errors, phases, and request bookkeeping.
    pub lifecycle: SearchLifecycleState,
    /// The result payload and its presentation state.
    pub result: ResultDataState,
    /// Chrome derived from the last-executed criteria.
    pub ui: UiChromeState,
}
