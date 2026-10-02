// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Pure table view model: encapsulates sorting and filtering orchestration.
//!
//! The result set arrives as a [`ColumnarResultSet`] -- every row the endpoint
//! returned, stored by column -- and **stays that way**. Nothing here materialises
//! a [`CompoundEntry`](lotus_model::CompoundEntry) for more than the rows on
//! screen; the view model is a list of row offsets and a sort state, and the
//! rows themselves are read out of the set when the virtualiser asks for a
//! window.
//!
//! Two steps, in increasing order of cost, and the component memoises them at that
//! granularity:
//!
//! 1. [`apply_sort`] — pick the index order. Runs when the sort changes; the
//!    sort-index cache is shared, not rebuilt.
//! 2. [`filter_order`] — drop the rows the column filters exclude. Runs when the
//!    filters change. Applied *after* sorting so the visible order is the order
//!    asked for, with holes removed, rather than the filter's order.
//!
//! Filtering compiles to one bitmap per constrained dictionary, so a keystroke
//! costs a pass over the dictionaries and four bit tests per row rather than a
//! string comparison per row. See `ColumnarResultSet::plan_filter`.

use super::sort_model::{SortIndexCache, build_sort_index_cache, indices_for_sort};
use crate::filters::ColumnFilters;
use crate::sort::SortState;
use lotus_model::{ColumnarResultSet, FilterPlan};
use std::sync::Arc;

/// Complete prepared state for rendering a results table.
#[derive(Clone, Debug)]
pub(super) struct TableViewModel {
    /// The result set, shared with the state that holds it.
    pub(super) set: Arc<ColumnarResultSet>,
    /// Row offsets in display order: sorted, then filtered.
    pub(super) order: Arc<[u32]>,
    /// Current sort state (needed for table header and context).
    pub(super) sort_state: SortState,
}

/// Compared by the set's identity and the order's contents, so a re-render with
/// neither changed is cheap and a new result set always is not equal to the old
/// one.
impl PartialEq for TableViewModel {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.set, &other.set)
            && self.order == other.order
            && self.sort_state == other.sort_state
    }
}

impl TableViewModel {
    /// How many rows survive the current filters — the number the user sees,
    /// which is not the number of rows the search returned.
    #[must_use]
    pub(super) fn visible_row_count(&self) -> usize {
        self.order.len()
    }
}

/// Preparation-only step: the sort-index cache, with no order chosen yet.
///
/// Separated so that a filter change does not throw away the sorted order, and a
/// sort change does not rebuild the cache.
#[derive(Clone, Debug)]
pub(super) struct PreparedTableState {
    /// The result set being shown.
    pub(super) set: Arc<ColumnarResultSet>,
    /// The lazily-populated per-column sort orders.
    pub(super) sort_cache: SortIndexCache,
}

/// Equal when the two point at the same result set.
///
/// The cache compares its own contents by pointer for the same reason: a new
/// result set has to invalidate every cached sort order, and the only reliable way
/// to notice is identity, not equality.
impl PartialEq for PreparedTableState {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.set, &other.set)
    }
}

/// Build only the sort-index cache. Memoized independently of sort state.
#[must_use]
pub(super) fn prepare_table_state(set: Arc<ColumnarResultSet>) -> PreparedTableState {
    let sort_cache = build_sort_index_cache(Arc::clone(&set));
    PreparedTableState { set, sort_cache }
}

/// Build a [`TableViewModel`] from an already-prepared [`PreparedTableState`] and sort state.
/// Only re-runs index selection when sort changes; the cache is untouched.
#[must_use]
pub(super) fn apply_sort(state: &PreparedTableState, sort: SortState) -> TableViewModel {
    TableViewModel {
        order: indices_for_sort(&state.sort_cache, sort),
        set: Arc::clone(&state.set),
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
pub(super) fn filter_order(
    set: &ColumnarResultSet,
    order: &Arc<[u32]>,
    filters: &ColumnFilters,
) -> Arc<[u32]> {
    if !filters.is_active() {
        return Arc::clone(order);
    }
    let plan = set.plan_filter(&filters.to_spec());
    Arc::from(filter_with(&plan, set, order).into_boxed_slice())
}

/// Keep the rows of `order` that `plan` admits, preserving order.
fn filter_with(plan: &FilterPlan, set: &ColumnarResultSet, order: &[u32]) -> Vec<u32> {
    order
        .iter()
        .copied()
        .filter(|offset| plan.accepts(set, *offset as usize))
        .collect()
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
        order: filter_order(&state.set, &sorted.order, filters),
        ..sorted
    }
}

#[cfg(test)]
#[path = "table_view_model/tests.rs"]
mod tests;
