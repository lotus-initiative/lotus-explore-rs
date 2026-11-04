// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use lotus_model::ColumnarResultSet;
#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

/// How many finished result sets are kept.
pub const MAX_CACHED_SETS: usize = 1;

/// How many recent dictionary seed sets are retained across consecutive searches.
const MAX_SEED_SETS: usize = 4;

/// The finished sets, keyed by the query text that produced them.
pub struct SetCache {
    sets: HashMap<String, Arc<ColumnarResultSet>>,
    /// The finished set with the largest entity dictionary seen in this session,
    /// retained even when a smaller search replaces the most-recent entry in `sets`
    /// so subsequent searches can still seed their dictionaries without re-querying.
    richest: Option<Arc<ColumnarResultSet>>,
    /// Additional dictionary entities captured during an unconstrained global
    /// dictionary stream (`>50,000` QIDs) that did not occur in that search's
    /// own rows. Together with `richest`, this holds the complete LOTUS compound
    /// and reference dictionary for the session.
    extra_seed: Option<Arc<ColumnarResultSet>>,
    /// Recent finished sets retained for seeding across distinct clades.
    seed_sets: Vec<Arc<ColumnarResultSet>>,
}

impl SetCache {
    pub(crate) fn new() -> Self {
        Self {
            sets: HashMap::new(),
            richest: None,
            extra_seed: None,
            seed_sets: Vec::new(),
        }
    }

    // Only the tests below read this, so it is not compiled into a binary
    // that has no tests to run.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.sets.len()
    }

    pub(crate) fn get(&self, key: &str) -> Option<Arc<ColumnarResultSet>> {
        self.sets.get(key).map(Arc::clone)
    }

    #[cfg(test)]
    pub(crate) fn any(&self) -> Option<Arc<ColumnarResultSet>> {
        self.richest
            .as_ref()
            .or_else(|| self.sets.values().next())
            .map(Arc::clone)
    }

    pub(crate) fn has_broad_seed(&self) -> bool {
        self.extra_seed.is_some()
            || self
                .richest
                .as_ref()
                .is_some_and(|set| set.stats().n_compounds >= 50_000)
    }

    pub(crate) fn store_extra_seed(&mut self, set: Arc<ColumnarResultSet>) {
        self.extra_seed = Some(set);
    }

    pub(crate) fn seed_reader(&self, reader: &mut lotus_query::CsvColumnarReader) {
        if let Some(richest) = &self.richest {
            reader.seed_from(richest);
        }
        if let Some(extra) = &self.extra_seed {
            reader.seed_from(extra);
        }
        for set in &self.seed_sets {
            reader.seed_from(set);
        }
    }

    pub(crate) fn insert(&mut self, key: String, set: Arc<ColumnarResultSet>) {
        let stats = set.stats();
        let weight = stats.n_compounds + stats.n_references;
        let is_richer = self.richest.as_ref().is_none_or(|prev| {
            let prev_stats = prev.stats();
            weight >= prev_stats.n_compounds + prev_stats.n_references
        });
        if is_richer && weight > 0 {
            self.richest = Some(Arc::clone(&set));
        }
        if weight > 0 && !self.seed_sets.iter().any(|prev| Arc::ptr_eq(prev, &set)) {
            if self.seed_sets.len() >= MAX_SEED_SETS {
                self.seed_sets.remove(0);
            }
            self.seed_sets.push(Arc::clone(&set));
        }
        if self.sets.len() >= MAX_CACHED_SETS {
            self.sets.clear();
        }
        self.sets.insert(key, set);
    }
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static SETS: RefCell<SetCache> = RefCell::new(SetCache::new());
}

/// The finished set for `key`, if one is cached.
///
/// A hit hands back the same `Arc` the previous render used, which is what
/// makes back-navigation free: no query, no parse, and no second copy.
#[cfg(target_arch = "wasm32")]
pub fn take_cached_set(key: &str) -> Option<Arc<ColumnarResultSet>> {
    SETS.with(|c| c.borrow().get(key))
}

/// True when the session cache already holds a global or broad dictionary
/// (`>=50,000` compounds or an unconstrained dictionary overflow set).
#[cfg(target_arch = "wasm32")]
pub fn has_broad_seed_set() -> bool {
    SETS.with(|c| c.borrow().has_broad_seed())
}

/// Seed `reader` from all retained dictionaries in the session cache.
#[cfg(target_arch = "wasm32")]
pub fn seed_reader_from_cache(reader: &mut lotus_query::CsvColumnarReader) {
    SETS.with(|c| c.borrow().seed_reader(reader));
}

/// Retain extra dictionary entities captured during an unconstrained dictionary stream.
#[cfg(target_arch = "wasm32")]
pub fn store_extra_seed_set(set: Arc<ColumnarResultSet>) {
    SETS.with(|c| c.borrow_mut().store_extra_seed(set));
}

/// Keep the finished set for the next identical navigation.
#[cfg(target_arch = "wasm32")]
pub fn store_cached_set(key: String, set: Arc<ColumnarResultSet>) {
    SETS.with(|c| c.borrow_mut().insert(key, set));
}

#[cfg(test)]
#[path = "cache_impl/tests.rs"]
mod tests;
