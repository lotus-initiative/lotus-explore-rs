// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! `O(D log D + N)` dictionary-rank counting sorts for [`ColumnarResultSet`].

use std::sync::Arc;

use super::ColumnarResultSet;
use super::qid_dict::slot_to_id;

impl ColumnarResultSet {
    /// Ascending row indices sorted by compound label (`None` first, ties broken
    /// by ascending row index), computed via dictionary-rank counting sort in
    /// `O(D log D + N)`.
    #[must_use]
    pub fn sort_by_name(&self) -> Arc<[u32]> {
        sort_rows_by_dictionary(
            &self.compound_ids,
            self.compounds.len(),
            |slot| self.compound_names.get(slot),
            Ord::cmp,
        )
    }

    /// Ascending row indices sorted by compound mass (`None` first, ties broken
    /// by ascending row index), computed via dictionary-rank counting sort in
    /// `O(D log D + N)`.
    #[must_use]
    pub fn sort_by_mass(&self) -> Arc<[u32]> {
        sort_rows_by_dictionary(
            &self.compound_ids,
            self.compounds.len(),
            |slot| self.compound_mass(slot_to_id(slot)),
            f64::total_cmp,
        )
    }

    /// Ascending row indices sorted by molecular formula (`None` first, ties
    /// broken by ascending row index), computed via dictionary-rank counting
    /// sort in `O(D log D + N)`.
    #[must_use]
    pub fn sort_by_formula(&self) -> Arc<[u32]> {
        sort_rows_by_dictionary(
            &self.compound_ids,
            self.compounds.len(),
            |slot| self.compound_formulas.get(slot),
            Ord::cmp,
        )
    }

    /// Ascending row indices sorted by taxon scientific name (`None` first, ties
    /// broken by ascending row index), computed via dictionary-rank counting
    /// sort in `O(D log D + N)`.
    #[must_use]
    pub fn sort_by_taxon_name(&self) -> Arc<[u32]> {
        sort_rows_by_dictionary(
            &self.taxon_ids,
            self.taxa.len(),
            |slot| self.taxon_names.get(slot),
            Ord::cmp,
        )
    }

    /// Ascending row indices sorted by reference publication year (`None` first,
    /// ties broken by ascending row index), computed via dictionary-rank
    /// counting sort in `O(D log D + N)`.
    #[must_use]
    pub fn sort_by_pub_year(&self) -> Arc<[u32]> {
        sort_rows_by_dictionary(
            &self.reference_ids,
            self.references.len(),
            |slot| self.reference_year(slot_to_id(slot)),
            Ord::cmp,
        )
    }

    /// Ascending row indices sorted by reference title (`None` first, ties
    /// broken by ascending row index), computed via dictionary-rank counting
    /// sort in `O(D log D + N)`.
    #[must_use]
    pub fn sort_by_ref_title(&self) -> Arc<[u32]> {
        sort_rows_by_dictionary(
            &self.reference_ids,
            self.references.len(),
            |slot| self.reference_titles.get(slot),
            Ord::cmp,
        )
    }
}

/// Sort `0..row_ids.len()` by a dictionary-keyed value in `O(D log D + N)` using
/// counting sort over dictionary ranks.
fn sort_rows_by_dictionary<K>(
    row_ids: &[u32],
    dict_len: usize,
    slot_key: impl Fn(usize) -> Option<K>,
    cmp: impl Fn(&K, &K) -> std::cmp::Ordering,
) -> Arc<[u32]> {
    let mut present: Vec<(usize, K)> = (0..dict_len)
        .filter_map(|slot| slot_key(slot).map(|key| (slot, key)))
        .collect();
    present.sort_unstable_by(|a, b| cmp(&a.1, &b.1));

    let mut slot_rank = vec![0usize; dict_len];
    let mut buckets = 1usize;
    for group in present.chunk_by(|a, b| cmp(&a.1, &b.1).is_eq()) {
        for &(slot, _) in group {
            if let Some(cell) = slot_rank.get_mut(slot) {
                *cell = buckets;
            }
        }
        buckets = buckets.saturating_add(1);
    }

    let mut offsets = vec![0usize; buckets];
    for &id in row_ids {
        let rank = slot_rank.get(id as usize).copied().unwrap_or(0);
        if let Some(count) = offsets.get_mut(rank) {
            *count = count.saturating_add(1);
        }
    }
    let mut sum = 0usize;
    for count in &mut offsets {
        let start = sum;
        sum = sum.saturating_add(*count);
        *count = start;
    }

    let mut out = vec![0u32; row_ids.len()];
    for (row, &id) in row_ids.iter().enumerate() {
        let rank = slot_rank.get(id as usize).copied().unwrap_or(0);
        if let Some(cursor) = offsets.get_mut(rank) {
            if let Some(slot) = out.get_mut(*cursor) {
                *slot = slot_to_id(row);
            }
            *cursor = cursor.saturating_add(1);
        }
    }
    Arc::from(out.into_boxed_slice())
}
