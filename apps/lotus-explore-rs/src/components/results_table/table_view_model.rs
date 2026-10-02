// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Pure table view model: encapsulates all preparation, sorting and filtering
//! orchestration.
//!
//! Three steps, in increasing order of cost, and the component memoises them at
//! that granularity:
//!
//! 1. [`prepare_table_state`] — row text derivation and the sort-index cache.
//!    Runs only when the entries themselves change.
//! 2. [`apply_sort`] — pick the index order. Runs when the sort changes; the
//!    prepared rows are shared, not rebuilt.
//! 3. [`filter_indices`] — drop the rows the column filters exclude. Runs when
//!    the filters change. Applied *after* sorting so the visible order is the
//!    order asked for, with holes removed, rather than the filter's order.

use super::row_cells::{PreparedRow, prepare_rows};
use super::sort_model::{SortIndexCache, build_sort_index_cache, indices_for_sort};
use crate::filters::ColumnFilters;
use crate::sort::SortState;
use lotus_model::{CompoundEntry, Rows};
use std::sync::Arc;

/// Complete prepared state for rendering a results table.
#[derive(Clone, PartialEq, Debug)]
pub(super) struct TableViewModel {
    /// Pre-formatted row data (derived from entries).
    pub(super) prepared_rows: Arc<[PreparedRow]>,
    /// Row offsets in display order: sorted, then filtered.
    pub(super) sorted_indices: Arc<[u32]>,
    /// Current sort state (needed for table header and context).
    pub(super) sort_state: SortState,
}

impl TableViewModel {
    /// How many rows survive the current filters — the number the user sees,
    /// which is not the number of rows the search returned.
    #[must_use]
    pub(super) fn visible_row_count(&self) -> usize {
        self.sorted_indices.len()
    }
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
/// Separated so that sort-state and filter changes don't re-run the expensive
/// row preparation.
#[derive(Clone, PartialEq, Debug)]
pub(super) struct PreparedTableState {
    pub(super) rows: Rows,
    pub(super) prepared_rows: Arc<[PreparedRow]>,
    pub(super) sort_cache: SortIndexCache,
}

/// Build only the row preparation and sort-index cache from entries.
/// This should be memoized independently of sort state.
#[must_use]
pub(super) fn prepare_table_state(rows: Rows) -> PreparedTableState {
    let prepared_rows = prepare_rows(rows.as_ref());
    let sort_cache = build_sort_index_cache(rows.clone());
    PreparedTableState {
        rows,
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

/// Narrow `order` to the rows that survive `filters`, preserving the order.
///
/// Filtering the sorted order rather than sorting the filtered rows is the point:
/// a user who sorts by mass and then types a taxon name still wants mass order,
/// not name order, and recomputing the sort would also throw away the cached
/// index that made sorting cheap.
///
/// An empty filter set returns the order untouched, so the common case costs one
/// comparison and allocates nothing.
#[must_use]
pub(super) fn filter_indices(
    rows: &[CompoundEntry],
    order: &Arc<[u32]>,
    filters: &ColumnFilters,
) -> Arc<[u32]> {
    let compiled = crate::filters::CompiledFilters::new(filters);
    if !compiled.is_active() {
        return order.clone();
    }

    let kept = order
        .iter()
        .copied()
        .filter(|offset| {
            rows.get(*offset as usize)
                .is_some_and(|entry| compiled.matches(entry))
        })
        .collect::<Vec<u32>>();

    Arc::from(kept.into_boxed_slice())
}

/// Sort, then filter, into the final display order.
#[must_use]
pub(super) fn apply_sort_and_filters(
    state: &PreparedTableState,
    sort: SortState,
    filters: &ColumnFilters,
) -> TableViewModel {
    let sorted = apply_sort(state, sort);
    TableViewModel {
        sorted_indices: filter_indices(state.rows.as_ref(), &sorted.sorted_indices, filters),
        ..sorted
    }
}

#[cfg(test)]
#[path = "table_view_model/tests.rs"]
mod tests;
