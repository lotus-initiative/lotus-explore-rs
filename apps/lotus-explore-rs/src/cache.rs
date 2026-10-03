// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! In-browser result cache.
//!
//! # What this used to hold
//!
//! Up to eight raw CSV response bodies, keyed by query, with whole-window
//! eviction. It was a cache of *payloads*, and it had no bound on payload size —
//! so eight wide searches could sit in memory at eight times the peak response,
//! which is the opposite of what a memory budget needs.
//!
//! What it holds now is the finished [`lotus_model::ColumnarResultSet`], which is the
//! thing the table is built from anyway. Sharing one costs an `Arc` and nothing
//! more, so a cached entry is not a second copy of the result -- and the bound is
//! one, because a cache exists to make the back button free rather than to hold a
//! history.
//!
//! Caching the *set* rather than the body is what makes this possible at all: the
//! body is no longer in memory, so there is nothing else to cache.

#[cfg(any(test, target_arch = "wasm32"))]
mod cache_impl {
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
    mod tests {
        #![allow(clippy::expect_used)]

        use super::*;
        use lotus_model::CompoundEntry;

        fn set_of(names: &[&str]) -> Arc<ColumnarResultSet> {
            let rows: Vec<CompoundEntry> = names
                .iter()
                .enumerate()
                .map(|(i, name)| CompoundEntry {
                    compound_qid: Arc::from(format!("Q{}", 100 + i)),
                    name: Arc::from(*name),
                    ..Default::default()
                })
                .collect();
            Arc::new(ColumnarResultSet::from_entries(&rows))
        }

        #[test]
        fn a_stored_set_comes_back_unchanged() {
            let mut cache = SetCache::new();
            let set = set_of(&["Alpha", "Beta"]);
            cache.insert("SELECT ?s WHERE {}".to_owned(), Arc::clone(&set));

            let hit = cache.get("SELECT ?s WHERE {}").expect("a stored set");

            assert_eq!(cache.len(), 1);
            assert_eq!(hit.row_count(), 2);
            assert!(
                Arc::ptr_eq(&hit, &set),
                "the same allocation, not a copy: that is the point of caching here"
            );
        }

        #[test]
        fn a_different_query_is_a_miss() {
            let mut cache = SetCache::new();
            cache.insert("one".to_owned(), set_of(&["Alpha"]));

            assert!(cache.get("two").is_none(), "keys are the whole query");
        }

        #[test]
        fn the_cache_holds_one_set() {
            // One, where the payload cache held eight. A set can be a hundred
            // megabytes; a cache that keeps eight of them is not a cache.
            let mut cache = SetCache::new();
            for i in 0..MAX_CACHED_SETS + 3 {
                cache.insert(format!("query-{i}"), set_of(&["Alpha"]));
            }

            assert_eq!(cache.len(), 1, "the window does not grow");
        }

        #[test]
        fn an_empty_set_is_still_cached() {
            // A search that matched nothing is a result, and re-running it on
            // every back-navigation is exactly the cost the cache exists to avoid.
            let mut cache = SetCache::new();
            cache.insert("nothing".to_owned(), set_of(&[]));

            let hit = cache.get("nothing").expect("an empty result is cached");

            assert_eq!(hit.row_count(), 0);
        }
    }
}

#[cfg(target_arch = "wasm32")]
// `unreachable_pub`-style narrowing requires `pub(crate)` for cross-module
// use; the nursery `redundant_pub_crate` suggestion (`pub`) would widen it.
#[allow(clippy::redundant_pub_crate)]
pub(crate) use cache_impl::{store_cached_set, take_cached_set};
