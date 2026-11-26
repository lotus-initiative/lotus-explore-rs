// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Slot-keyed interned string storage for sparse columns.

use std::sync::Arc;

use rustc_hash::FxHashMap;

use super::NO_VALUE;
use super::builder::non_empty;

/// Strings stored beside a dictionary entry, for columns that are mostly absent.
///
/// An `Option<Arc<str>>` column over two and a half million compounds costs 16
/// bytes per compound even when four fifths of them have no value. This costs
/// four, and allocates only for the values that exist.
#[derive(Debug, Default, Clone)]
pub struct SparseStrings {
    pub(super) values: Vec<Arc<str>>,
    /// Maps a value to its id in `values`, so a value repeated across a million
    /// rows is one allocation.
    ///
    /// Without this the column is not sparse: it stored a fresh copy per *row* and
    /// pointed the slot at the last, so a taxon name shared by half a million rows
    /// cost 37 MB to hold 135 distinct strings.
    index: FxHashMap<Arc<str>, u32>,
    pub(super) ids: Vec<u32>,
}

impl SparseStrings {
    /// An empty set of slots.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record `value` for dictionary slot `slot`, and return the id it resolved to.
    ///
    /// **One value per slot: the first non-empty write keeps it.** Later calls for a
    /// filled slot are ignored, *including when the value differs*.
    ///
    /// Later rows do often disagree. Measured on `Q21754` (32,160 rows), rows whose
    /// value differs from the first row for the same entity:
    ///
    /// | column | disagreeing rows |
    /// |---|---|
    /// | `ref_title` | 1,850 |
    /// | `compound_inchikey` | 1,315 |
    /// | `compound_smiles_conn` | 1,142 |
    /// | `ref_date` | 754 |
    /// | `compound_smiles_iso` | 154 |
    /// | `compound_mass` | 133 |
    /// | `ref_doi` | 4 |
    ///
    /// Causes are ordinary: a `P1476` title per language, an `InChIKey` per
    /// protonation or tautomer form, a mass per charge state. So dropping the second
    /// is right for this store -- a table cell holds one string -- but it is a
    /// decision, not an invariant. A store wanting every value needs a list per slot
    /// and a per-row index into it: a different type.
    ///
    /// The early return is kept anyway: interning per row made the build *slower
    /// than not interning at all*, because three million rows would hash three
    /// million copies of an already-present string. Reading `ids` instead of hashing
    /// is the win, and it holds whether or not the values agree.
    ///
    /// The id is returned rather than left to the caller because predicting where
    /// the interned value lands is error-prone: a wrong prediction for an already
    /// interned value grows `ids` without growing `values`, once per *call*. Three
    /// million repetitions of one value would cost twelve megabytes of slots
    /// describing nothing.
    pub fn set(&mut self, slot: usize, value: Option<&str>) -> u32 {
        if let Some(existing) = self.ids.get(slot)
            && *existing != NO_VALUE
        {
            return *existing;
        }
        let id = non_empty(value).map_or(NO_VALUE, |v| self.intern(v));
        if slot >= self.ids.len() {
            self.ids.resize(slot + 1, NO_VALUE);
        }
        if let Some(existing) = self.ids.get_mut(slot) {
            *existing = id;
        }
        id
    }

    /// The id for `value`, minting one if it is new.
    fn intern(&mut self, value: &str) -> u32 {
        if let Some(id) = self.index.get(value) {
            return *id;
        }
        let Ok(id) = u32::try_from(self.values.len()) else {
            return NO_VALUE;
        };
        if id == NO_VALUE {
            return NO_VALUE;
        }
        let shared: Arc<str> = Arc::from(value);
        self.values.push(Arc::clone(&shared));
        self.index.insert(shared, id);
        id
    }

    /// The distinct value `id` names, for a column read as a plain interning table.
    ///
    /// `get` takes a *slot*; this takes a *value id*. The two are the same number
    /// only when every slot holds a distinct value, which is true of the compound
    /// columns and false of an append-only one that has seen repeats.
    #[must_use]
    pub fn value(&self, id: usize) -> Option<&str> {
        self.values.get(id).map(AsRef::as_ref)
    }

    /// Intern `text` and return its value id, without touching any slot.
    ///
    /// For an append-only column where the id *is* the index into the distinct
    /// values. `set` is the wrong tool: its caller must choose a slot, which means
    /// predicting where the value will land, and that prediction is wrong for an
    /// already interned value -- wrong in the direction of a slot per call. Here
    /// the id comes from the interning table itself, so it cannot be wrong.
    pub fn intern_text(&mut self, text: &str) -> u32 {
        self.intern(text)
    }

    /// How many distinct values are interned.
    #[must_use]
    pub const fn distinct_len(&self) -> usize {
        self.values.len()
    }

    /// How many slots have values or have been filled.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.ids.len()
    }

    /// Whether no slot has been filled.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    fn value_for_slot(&self, slot: usize) -> Option<&Arc<str>> {
        let id = *self.ids.get(slot)?;
        if id == NO_VALUE {
            return None;
        }
        self.values.get(id as usize)
    }

    /// The value recorded for `slot`.
    #[must_use]
    pub fn get(&self, slot: usize) -> Option<&str> {
        self.value_for_slot(slot).map(AsRef::as_ref)
    }

    /// The shared value recorded for `slot`, cloned by reference count rather
    /// than by copying its text.
    #[must_use]
    pub fn get_arc(&self, slot: usize) -> Option<Arc<str>> {
        self.value_for_slot(slot).cloned()
    }
}
