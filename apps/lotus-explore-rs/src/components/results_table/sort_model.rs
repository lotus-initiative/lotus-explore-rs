// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::sort::{SortColumn, SortDir, SortState};
use lotus_model::CompoundEntry;
use std::cmp::Ordering;
use std::sync::{Arc, Mutex};

// --- Lazy sort index cache ---------------------------------------------------

/// The cache slot for a column: one per `SortColumn` variant, and the order is
/// what the match below says it is.
const fn sort_column_index(col: SortColumn) -> usize {
    match col {
        SortColumn::Name => 0,
        SortColumn::Mass => 1,
        SortColumn::Formula => 2,
        SortColumn::TaxonName => 3,
        SortColumn::PubYear => 4,
        SortColumn::RefTitle => 5,
    }
}

/// Number of sortable columns.
///
/// Derived from the match rather than written down beside it: a variant added to
/// `SortColumn` without a slot here would silently index past the end of the cache
/// array, and a literal `6` is exactly the kind of thing that survives adding a
/// seventh variant.
const NUM_SORT_COLS: usize = match SortColumn::all().last() {
    Some(last) => sort_column_index(*last) + 1,
    None => 0,
};

struct SortCacheInner {
    rows: Arc<[CompoundEntry]>,
    /// Ascending sort indices per column; `None` until first access.
    asc_by_col: Mutex<[Option<Arc<[u32]>>; NUM_SORT_COLS]>,
    /// Descending sort indices per column; derived from ascending once and then reused.
    desc_by_col: Mutex<[Option<Arc<[u32]>>; NUM_SORT_COLS]>,
}

/// Lazily-populated, cheaply-cloneable sort index cache.
/// Each column's ascending sort index is built on the first `indices_for_sort`
#[derive(Clone)]
pub(super) struct SortIndexCache(Arc<SortCacheInner>);

impl PartialEq for SortIndexCache {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0.rows, &other.0.rows)
    }
}

impl std::fmt::Debug for SortIndexCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SortIndexCache")
            .field("rows_len", &self.0.rows.len())
            .finish_non_exhaustive()
    }
}

/// Build a new lazy sort index cache backed by `rows`.
/// No sort work is performed here; indices are computed on first access per
/// column.
#[must_use]
pub(super) fn build_sort_index_cache(rows: Arc<[CompoundEntry]>) -> SortIndexCache {
    SortIndexCache(Arc::new(SortCacheInner {
        rows,
        asc_by_col: Mutex::new(Default::default()),
        desc_by_col: Mutex::new(Default::default()),
    }))
}

impl SortIndexCache {
    fn ascending_for(&self, col: SortColumn) -> Arc<[u32]> {
        let idx = sort_column_index(col);
        // Fast path: return the cached value while holding the lock briefly.
        {
            // Recover from poisoning (a panicking writer leaves valid data).
            let guard = self
                .0
                .asc_by_col
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(Some(cached)) = guard.get(idx) {
                return cached.clone();
            }
        }
        // Compute outside the lock so that other columns can be accessed
        // concurrently on native; on WASM the Mutex is a no-op anyway.
        let computed = build_sorted_indices_for_column(&self.0.rows, col);
        // Store and return; a benign race on native means two threads might
        // both compute the same column — both results are identical, so the
        // last writer's value is silently discarded by get_or_insert.
        // `if let Some` rather than `expect`: `expect_used` is denied, and this
        // is an invariant the type system cannot see -- `sort_column_index`
        // returns a `usize` with no upper bound. A variant added to `SortColumn`
        // without a slot in `all()` would land here, and the honest result is the
        // freshly computed indices with nothing cached, not a panic in a render
        // path.
        {
            let mut guard = self
                .0
                .asc_by_col
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match guard.get_mut(idx) {
                Some(slot) => slot.get_or_insert(computed).clone(),
                None => computed,
            }
        }
    }

    fn descending_for(&self, col: SortColumn) -> Arc<[u32]> {
        let idx = sort_column_index(col);
        // Fast path: return the cached value while holding the lock briefly.
        {
            // Recover from poisoning (a panicking writer leaves valid data).
            let guard = self
                .0
                .desc_by_col
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(Some(cached)) = guard.get(idx) {
                return cached.clone();
            }
        }

        let ascending = self.ascending_for(col);
        let computed = reversed_indices(&ascending);

        // `if let Some` rather than `expect`: `expect_used` is denied, and this
        // is an invariant the type system cannot see -- `sort_column_index`
        // returns a `usize` with no upper bound. A variant added to `SortColumn`
        // without a slot in `all()` would land here, and the honest result is the
        // freshly computed indices with nothing cached, not a panic in a render
        // path.
        {
            let mut guard = self
                .0
                .desc_by_col
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match guard.get_mut(idx) {
                Some(slot) => slot.get_or_insert(computed).clone(),
                None => computed,
            }
        }
    }
}

/// Returns the sorted index sequence for the given `SortState`.
#[must_use]
pub(super) fn indices_for_sort(cache: &SortIndexCache, sort: SortState) -> Arc<[u32]> {
    if sort.dir == SortDir::Asc {
        cache.ascending_for(sort.col)
    } else {
        cache.descending_for(sort.col)
    }
}

/// Test-only helper: directly build a sorted index from a slice without caching.
#[cfg(test)]
#[must_use]
pub(super) fn build_sorted_indices(rows: &[CompoundEntry], sort: SortState) -> Arc<[u32]> {
    let ascending = build_sorted_indices_for_column(rows, sort.col);
    if sort.dir == SortDir::Asc {
        ascending
    } else {
        reversed_indices(&ascending)
    }
}

#[allow(clippy::cast_possible_truncation)] // `rows.len()` far below `u32::MAX` for any displayable table
#[allow(clippy::indexing_slicing)] // `a`/`b` drawn from `0..rows.len()`, always in bounds
fn build_sorted_indices_for_column(rows: &[CompoundEntry], column: SortColumn) -> Arc<[u32]> {
    let mut idx: Vec<u32> = (0..rows.len() as u32).collect();
    idx.sort_by(|&a, &b| {
        compare_entries(&rows[a as usize], &rows[b as usize], column).then_with(|| a.cmp(&b))
    });
    Arc::from(idx.into_boxed_slice())
}

fn reversed_indices(indices: &[u32]) -> Arc<[u32]> {
    let mut reversed = Vec::with_capacity(indices.len());
    reversed.extend(indices.iter().rev().copied());
    Arc::from(reversed.into_boxed_slice())
}

fn compare_entries(a: &CompoundEntry, b: &CompoundEntry, column: SortColumn) -> Ordering {
    match column {
        SortColumn::Name => a.name.cmp(&b.name),
        SortColumn::Mass => a.mass.partial_cmp(&b.mass).unwrap_or(Ordering::Equal),
        SortColumn::Formula => a.formula.cmp(&b.formula),
        SortColumn::TaxonName => a.taxon_name.cmp(&b.taxon_name),
        SortColumn::PubYear => a.pub_year.cmp(&b.pub_year),
        SortColumn::RefTitle => a.ref_title.cmp(&b.ref_title),
    }
}

#[cfg(test)]
#[path = "sort_model/tests.rs"]
mod tests;
