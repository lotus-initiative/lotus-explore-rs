// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use lotus_model::ColumnarResultSet;
#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

/// How many finished result sets are kept.
pub const MAX_CACHED_SETS: usize = 1;

/// The finished sets, keyed by the query text that produced them.
pub struct SetCache {
    sets: HashMap<String, Arc<ColumnarResultSet>>,
}

impl SetCache {
    pub(crate) fn new() -> Self {
        Self {
            sets: HashMap::new(),
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

    pub(crate) fn insert(&mut self, key: String, set: Arc<ColumnarResultSet>) {
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
#[allow(clippy::redundant_pub_crate)]
pub(crate) fn take_cached_set(key: &str) -> Option<Arc<ColumnarResultSet>> {
    SETS.with(|c| c.borrow().get(key))
}

/// Keep the finished set for the next identical navigation.
#[cfg(target_arch = "wasm32")]
#[allow(clippy::redundant_pub_crate)]
pub(crate) fn store_cached_set(key: String, set: Arc<ColumnarResultSet>) {
    SETS.with(|c| c.borrow_mut().insert(key, set));
}

#[cfg(test)]
#[path = "cache_impl/tests.rs"]
mod tests;
