// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::models::{CompoundEntry, SortColumn, SortDir, SortState};
use std::cmp::Ordering;
use std::sync::{Arc, Mutex};

// --- Lazy sort index cache ---------------------------------------------------

/// Number of sortable columns (one slot per `SortColumn` variant).
const NUM_SORT_COLS: usize = 6;

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
    // `idx` comes from the exhaustive `sort_column_index` match, so it is
    // always `< NUM_SORT_COLS`; indexing a fixed-size array is safe here.
    #[allow(clippy::indexing_slicing)]
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
            if let Some(cached) = &guard[idx] {
                return cached.clone();
            }
        }
        // Compute outside the lock so that other columns can be accessed
        // concurrently on native; on WASM the Mutex is a no-op anyway.
        let computed = build_sorted_indices_for_column(&self.0.rows, col);
        // Store and return; a benign race on native means two threads might
        // both compute the same column — both results are identical, so the
        // last writer's value is silently discarded by get_or_insert.
        let mut guard = self
            .0
            .asc_by_col
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard[idx].get_or_insert(computed).clone()
    }

    // `idx` comes from the exhaustive `sort_column_index` match, so it is
    // always `< NUM_SORT_COLS`; indexing a fixed-size array is safe here.
    #[allow(clippy::indexing_slicing)]
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
            if let Some(cached) = &guard[idx] {
                return cached.clone();
            }
        }

        let ascending = self.ascending_for(col);
        let computed = reversed_indices(&ascending);

        let mut guard = self
            .0
            .desc_by_col
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard[idx].get_or_insert(computed).clone()
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
mod tests {
    use super::*;
    use std::sync::Arc;

    fn entry(
        name: &str,
        mass: Option<f64>,
        formula: Option<&str>,
        taxon_name: &str,
        pub_year: Option<i16>,
        ref_title: Option<&str>,
    ) -> CompoundEntry {
        CompoundEntry {
            compound_qid: Arc::<str>::from(format!("Q-{name}")),
            name: Arc::<str>::from(name),
            inchikey: None,
            smiles: None,
            mass,
            formula: formula.map(Arc::<str>::from),
            taxon_qid: Arc::<str>::from(format!("T-{taxon_name}")),
            taxon_name: Arc::<str>::from(taxon_name),
            reference_qid: Arc::<str>::from("R-1"),
            ref_title: ref_title.map(Arc::<str>::from),
            ref_doi: None,
            pub_year,
            statement: None,
        }
    }

    #[test]
    fn sorts_by_name_ascending_by_default() {
        let rows = vec![
            entry(
                "Gamma",
                Some(3.0),
                Some("C3"),
                "Taxon C",
                Some(2003),
                Some("Ref C"),
            ),
            entry(
                "Alpha",
                Some(1.0),
                Some("C1"),
                "Taxon A",
                Some(2001),
                Some("Ref A"),
            ),
            entry(
                "Beta",
                Some(2.0),
                Some("C2"),
                "Taxon B",
                Some(2002),
                Some("Ref B"),
            ),
        ];

        let order = build_sorted_indices(&rows, SortState::default());
        assert_eq!(order.as_ref(), &[1, 2, 0]);
    }

    #[test]
    fn sorts_by_mass_descending() {
        let rows = vec![
            entry(
                "Alpha",
                Some(10.0),
                Some("C1"),
                "Taxon A",
                Some(2001),
                Some("Ref A"),
            ),
            entry(
                "Beta",
                Some(30.0),
                Some("C2"),
                "Taxon B",
                Some(2002),
                Some("Ref B"),
            ),
            entry(
                "Gamma",
                Some(20.0),
                Some("C3"),
                "Taxon C",
                Some(2003),
                Some("Ref C"),
            ),
        ];

        let order = build_sorted_indices(
            &rows,
            SortState {
                col: SortColumn::Mass,
                dir: SortDir::Desc,
            },
        );
        assert_eq!(order.as_ref(), &[1, 2, 0]);
    }

    #[test]
    fn sorts_optional_reference_titles() {
        let rows = vec![
            entry(
                "Alpha",
                Some(10.0),
                Some("C1"),
                "Taxon A",
                Some(2001),
                Some("Zeta"),
            ),
            entry("Beta", Some(30.0), Some("C2"), "Taxon B", Some(2002), None),
            entry(
                "Gamma",
                Some(20.0),
                Some("C3"),
                "Taxon C",
                Some(2003),
                Some("Alpha"),
            ),
        ];

        let order = build_sorted_indices(
            &rows,
            SortState {
                col: SortColumn::RefTitle,
                dir: SortDir::Asc,
            },
        );
        assert_eq!(order.as_ref(), &[1, 2, 0]);
    }

    #[test]
    fn cache_returns_same_indices_as_direct_sort_for_asc_and_desc() {
        let rows = vec![
            entry(
                "Gamma",
                Some(3.0),
                Some("C3"),
                "Taxon C",
                Some(2003),
                Some("Ref C"),
            ),
            entry(
                "Alpha",
                Some(1.0),
                Some("C1"),
                "Taxon A",
                Some(2001),
                Some("Ref A"),
            ),
            entry(
                "Beta",
                Some(2.0),
                Some("C2"),
                "Taxon B",
                Some(2002),
                Some("Ref B"),
            ),
        ];

        let rows_arc: Arc<[CompoundEntry]> = Arc::from(rows.as_slice());
        let cache = build_sort_index_cache(rows_arc);
        let asc = indices_for_sort(
            &cache,
            SortState {
                col: SortColumn::Name,
                dir: SortDir::Asc,
            },
        );
        let desc = indices_for_sort(
            &cache,
            SortState {
                col: SortColumn::Name,
                dir: SortDir::Desc,
            },
        );

        assert_eq!(
            asc.as_ref(),
            build_sorted_indices(&rows, SortState::default()).as_ref()
        );
        assert_eq!(desc.as_ref(), &[0, 2, 1]);
    }

    #[test]
    fn descending_sort_is_exact_reverse_of_ascending_order() {
        let rows = vec![
            entry(
                "Alpha",
                Some(10.0),
                Some("C1"),
                "Taxon A",
                Some(2001),
                Some("Zeta"),
            ),
            entry(
                "Alpha",
                Some(10.0),
                Some("C1"),
                "Taxon A",
                Some(2001),
                Some("Alpha"),
            ),
            entry(
                "Gamma",
                Some(20.0),
                Some("C3"),
                "Taxon C",
                Some(2003),
                Some("Beta"),
            ),
        ];

        let asc = build_sorted_indices(
            &rows,
            SortState {
                col: SortColumn::RefTitle,
                dir: SortDir::Asc,
            },
        );
        let desc = build_sorted_indices(
            &rows,
            SortState {
                col: SortColumn::RefTitle,
                dir: SortDir::Desc,
            },
        );

        let expected_desc: Vec<u32> = asc.iter().rev().copied().collect();
        assert_eq!(desc.as_ref(), expected_desc.as_slice());
    }

    #[test]
    fn descending_indices_are_cached_per_column() {
        let rows = vec![
            entry(
                "Gamma",
                Some(3.0),
                Some("C3"),
                "Taxon C",
                Some(2003),
                Some("Ref C"),
            ),
            entry(
                "Alpha",
                Some(1.0),
                Some("C1"),
                "Taxon A",
                Some(2001),
                Some("Ref A"),
            ),
            entry(
                "Beta",
                Some(2.0),
                Some("C2"),
                "Taxon B",
                Some(2002),
                Some("Ref B"),
            ),
        ];

        let rows_arc: Arc<[CompoundEntry]> = Arc::from(rows.as_slice());
        let cache = build_sort_index_cache(rows_arc);
        let first = indices_for_sort(
            &cache,
            SortState {
                col: SortColumn::Name,
                dir: SortDir::Desc,
            },
        );
        let second = indices_for_sort(
            &cache,
            SortState {
                col: SortColumn::Name,
                dir: SortDir::Desc,
            },
        );

        assert!(Arc::ptr_eq(&first, &second));
    }
}
