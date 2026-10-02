// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::export::SparqlEndpoint;
use crate::features::explore::types::LookupNotice;
use crate::filters::ColumnFilters;
use crate::sort::{SortColumn, SortDir};
use std::sync::Arc;

use super::super::ResultDataState;
use lotus_model::{CompoundEntry, DatasetStats};

pub(super) struct SearchSuccessPayload {
    pub rows: Vec<CompoundEntry>,
    pub qid: Option<String>,
    pub warnings: Vec<LookupNotice>,
    pub query: String,
    pub total_matches: Option<usize>,
    pub total_stats: Option<DatasetStats>,
    pub display_capped_rows: bool,
    pub query_hash: Arc<str>,
    pub result_hash: Arc<str>,
    pub metadata_json: Arc<str>,
    pub endpoint: SparqlEndpoint,
}

pub(super) fn reset_for_new_search(state: &mut ResultDataState) {
    *state = ResultDataState::default();
}

pub(super) fn clear(state: &mut ResultDataState) {
    *state = ResultDataState::default();
}

pub(super) fn search_succeeded(state: &mut ResultDataState, payload: SearchSuccessPayload) {
    state.entries = Arc::from(payload.rows.into_boxed_slice());
    state.lookup_notices = payload.warnings;
    state.resolved_qid = payload.qid.map(Arc::from);
    state.query_hash = Some(payload.query_hash);
    state.result_hash = Some(payload.result_hash);
    // If WDQS fallback occurred, replace with transformed query
    if matches!(payload.endpoint, crate::export::SparqlEndpoint::Wdqs) {
        if let Some(transformed) = crate::repositories::get_wdqs_transformed_query() {
            state.sparql_query = Some(Arc::<str>::from(transformed));
        }
    } else {
        state.sparql_query = Some(Arc::<str>::from(payload.query));
    }
    state.metadata_json = Some(payload.metadata_json);
    state.total_matches = payload.total_matches;
    state.total_stats = payload.total_stats;
    state.display_capped_rows = payload.display_capped_rows;
    state.endpoint = payload.endpoint;
}

pub(super) fn sort_toggled(state: &mut ResultDataState, column: SortColumn) {
    if state.sort.col == column {
        state.sort.dir = match state.sort.dir {
            SortDir::Asc => SortDir::Desc,
            SortDir::Desc => SortDir::Asc,
        };
    } else {
        state.sort.col = column;
        state.sort.dir = SortDir::Asc;
    }
}

pub(super) fn filters_changed(state: &mut ResultDataState, filters: ColumnFilters) {
    state.filters = filters;
}

pub(super) fn filters_cleared(state: &mut ResultDataState) {
    state.filters.clear();
}
