// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
use crate::features::explore::types::{DomainError, QueryPhase, TaxonWarning};
use crate::sort::SortState;
use lotus_model::{CompoundEntry, DatasetStats, Rows, SearchCriteria};
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
#[derive(Clone, PartialEq, Eq)]
pub struct SearchLifecycleState {
    pub loading: bool,
    pub error: Option<DomainError>,
    pub query_phase: QueryPhase,
    pub searched_once: bool,
    pub download_only_mode: bool,
    pub download_dispatching: bool,
    pub search_request_token: u64,
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
        }
    }
}

/// Result payload and presentation state. Changes here re-render the results
/// table, toolbar, header-meta row, and taxon notice.
#[derive(Clone, PartialEq)]
pub struct ResultDataState {
    pub entries: Rows,
    pub taxon_notice: Option<TaxonWarning>,
    pub resolved_qid: Option<Arc<str>>,
    pub query_hash: Option<Arc<str>>,
    pub result_hash: Option<Arc<str>>,
    pub sparql_query: Option<Arc<str>>,
    pub metadata_json: Option<Arc<str>>,
    pub total_matches: Option<usize>,
    pub total_stats: Option<DatasetStats>,
    pub display_capped_rows: bool,
    pub sort: SortState,
    /// The SPARQL endpoint used (Qlever by default, WDQS on fallback from 502)
    pub endpoint: crate::export::SparqlEndpoint,
}

impl Default for ResultDataState {
    fn default() -> Self {
        Self {
            entries: Arc::<[CompoundEntry]>::from([]),
            taxon_notice: None,
            resolved_qid: None,
            query_hash: None,
            result_hash: None,
            sparql_query: None,
            metadata_json: None,
            total_matches: None,
            total_stats: None,
            display_capped_rows: false,
            sort: SortState::default(),
            endpoint: crate::export::SparqlEndpoint::Qlever,
        }
    }
}

/// UI chrome and the last-executed criteria snapshot. Changes here re-render
/// the query toolbar.
#[derive(Clone, PartialEq)]
pub struct UiChromeState {
    pub executed_criteria: SearchCriteria,
}

impl Default for UiChromeState {
    fn default() -> Self {
        Self {
            executed_criteria: SearchCriteria::up_to_year(crate::clock::current_year()),
        }
    }
}

#[derive(Clone, PartialEq, Default)]
pub struct ExploreState {
    pub lifecycle: SearchLifecycleState,
    pub result: ResultDataState,
    pub ui: UiChromeState,
}
