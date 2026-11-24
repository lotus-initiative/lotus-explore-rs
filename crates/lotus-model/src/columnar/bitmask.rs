// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Fixed-width word-packed bitset over dictionary IDs.

use super::NO_VALUE;
use super::qid_dict::slot_to_id;

/// A fixed-size set of ids, stored as words.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Bitmask {
    pub(super) words: Vec<u64>,
}

impl Bitmask {
    /// A mask over `len` ids, all clear.
    #[must_use]
    pub fn with_len(len: usize) -> Self {
        Self {
            words: vec![0; len.div_ceil(64)],
        }
    }

    /// Add `id`, ignoring [`NO_VALUE`] and out-of-range ids.
    pub fn insert(&mut self, id: u32) {
        if id == NO_VALUE {
            return;
        }
        if let Some(word) = self.words.get_mut((id / 64) as usize) {
            *word |= 1u64 << (id % 64);
        }
    }

    /// Whether `id` is in the mask. [`NO_VALUE`] never is.
    #[must_use]
    pub fn contains(&self, id: u32) -> bool {
        if id == NO_VALUE {
            return false;
        }
        self.words
            .get((id / 64) as usize)
            .is_some_and(|w| w & (1u64 << (id % 64)) != 0)
    }

    /// How many ids are in the mask.
    #[must_use]
    pub fn len(&self) -> usize {
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Whether the mask holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|w| *w == 0)
    }
}

/// A bitmap over `len` dictionary slots, from a per-entry predicate.
pub(super) fn build_mask(len: usize, mut keep: impl FnMut(usize, u32) -> bool) -> Bitmask {
    let mut mask = Bitmask::with_len(len);
    for slot in 0..len {
        let id = slot_to_id(slot);
        if keep(slot, id) {
            mask.insert(id);
        }
    }
    mask
}
