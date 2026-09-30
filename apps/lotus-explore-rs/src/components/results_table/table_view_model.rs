// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Pure table view model: encapsulates all preparation and sorting orchestration.

use super::row_cells::{PreparedRow, prepare_rows};
use super::sort_model::{SortIndexCache, build_sort_index_cache, indices_for_sort};
use crate::sort::SortState;
use lotus_model::Rows;
use std::sync::Arc;

/// Complete prepared state for rendering a results table.
#[derive(Clone, PartialEq, Debug)]
pub(super) struct TableViewModel {
    /// Pre-formatted row data (derived from entries).
    pub(super) prepared_rows: Arc<[PreparedRow]>,
    /// Index order according to current sort state.
    pub(super) sorted_indices: Arc<[u32]>,
    /// Current sort state (needed for table header and context).
    pub(super) sort_state: SortState,
}

/// Builds a complete table view model from raw entries and sort state.
/// This is the primary boundary: raw data → fully-prepared view model.
/// All preparation and caching logic is encapsulated here.
#[must_use]
#[cfg(test)]
pub(super) fn build_table_view_model(rows: &Rows, sort_state: SortState) -> TableViewModel {
    let prepared_rows = prepare_rows(rows.as_ref());
    let sort_index_cache = build_sort_index_cache(rows.clone());
    let sorted_indices = indices_for_sort(&sort_index_cache, sort_state);

    TableViewModel {
        prepared_rows,
        sorted_indices,
        sort_state,
    }
}

/// Preparation-only step: row text derivation without sort ordering.
/// Separated so that sort-state changes don't re-run the expensive row preparation.
#[derive(Clone, PartialEq, Debug)]
pub(super) struct PreparedTableState {
    pub(super) prepared_rows: Arc<[PreparedRow]>,
    pub(super) sort_cache: SortIndexCache,
}

/// Build only the row preparation and sort-index cache from entries.
/// This should be memoized independently of sort state.
#[must_use]
pub(super) fn prepare_table_state(rows: Rows) -> PreparedTableState {
    let prepared_rows = prepare_rows(rows.as_ref());
    let sort_cache = build_sort_index_cache(rows);
    PreparedTableState {
        prepared_rows,
        sort_cache,
    }
}

/// Build a [`TableViewModel`] from an already-prepared [`PreparedTableState`] and sort state.
/// Only re-runs index selection when sort changes; row preparation is skipped.
#[must_use]
pub(super) fn apply_sort(state: &PreparedTableState, sort: SortState) -> TableViewModel {
    let sorted_indices = indices_for_sort(&state.sort_cache, sort);
    TableViewModel {
        prepared_rows: state.prepared_rows.clone(),
        sorted_indices,
        sort_state: sort,
    }
}

#[cfg(test)]
#[path = "table_view_model/tests.rs"]
mod tests;
