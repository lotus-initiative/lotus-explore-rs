// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! In-browser result cache.

#[cfg(any(test, target_arch = "wasm32"))]
mod cache_impl {
    use lotus::transport::ResponseBody;
    #[cfg(target_arch = "wasm32")]
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// Upper bound on result pages kept per browser session.
    pub const MAX_CACHED_RESULT_PAGES: usize = 8;

    pub struct ResultCache {
        entries: HashMap<String, ResponseBody>,
    }

    impl ResultCache {
        pub(crate) fn new() -> Self {
            Self {
                entries: HashMap::new(),
            }
        }

        pub(crate) fn len(&self) -> usize {
            self.entries.len()
        }

        pub(crate) fn get(&self, key: &str) -> Option<&ResponseBody> {
            self.entries.get(key)
        }

        pub(crate) fn insert(&mut self, key: String, bytes: ResponseBody) {
            // Simple bounded eviction: once full, drop everything and start a
            // fresh session-sized window (good enough for back/repeat nav).
            if self.entries.len() >= MAX_CACHED_RESULT_PAGES {
                self.entries.clear();
            }
            self.entries.insert(key, bytes);
        }
    }

    #[cfg(target_arch = "wasm32")]
    thread_local! {
        static CACHE: RefCell<ResultCache> = RefCell::new(ResultCache::new());
    }

    /// Returns a cached result page if present (cache hit).
    #[cfg(target_arch = "wasm32")]
    // `unreachable_pub`-style narrowing requires `pub(crate)` for cross-module
    // use; the nursery `redundant_pub_crate` suggestion (`pub`) would widen it.
    #[allow(clippy::redundant_pub_crate)]
    pub(crate) fn get_cached(key: &str) -> Option<ResponseBody> {
        CACHE.with(|c| c.borrow().get(key).cloned())
    }

    /// Stores a result page for the next identical request (cache miss).
    #[cfg(target_arch = "wasm32")]
    // `unreachable_pub`-style narrowing requires `pub(crate)` for cross-module
    // use; the nursery `redundant_pub_crate` suggestion (`pub`) would widen it.
    #[allow(clippy::redundant_pub_crate)]
    pub(crate) fn store_cached(key: String, bytes: ResponseBody) {
        CACHE.with(|c| c.borrow_mut().insert(key, bytes));
    }

    #[cfg(test)]
    mod tests {
        #![allow(clippy::expect_used)]

        use super::*;
        use lotus::state::build_search_cache_key;

        fn body() -> ResponseBody {
            ResponseBody::from_static(b"compound,compoundLabel\nQ1,One\n")
        }

        #[test]
        fn cache_hit_after_insert() {
            let mut c = ResultCache::new();
            let key = build_search_cache_key("SELECT ?s WHERE { ?s ?p ?o }", 100, true);
            c.insert(key.clone(), body());
            assert_eq!(c.len(), 1);
            let hit = c.get(&key).expect("page should be cached");
            assert_eq!(hit.as_ref(), &b"compound,compoundLabel\nQ1,One\n"[..]);
            assert!(c.get("nope").is_none(), "missing key must miss");
        }

        #[test]
        fn cache_evicts_when_full() {
            let mut c = ResultCache::new();
            for i in 0..MAX_CACHED_RESULT_PAGES {
                c.insert(format!("k{i}"), body());
            }
            assert_eq!(c.len(), MAX_CACHED_RESULT_PAGES);
            c.insert("overflow".to_string(), body());
            assert!(c.get("overflow").is_some(), "new entry must be stored");
            assert!(
                c.get("k0").is_none(),
                "overflow must evict the previous window"
            );
        }

        #[test]
        fn cache_keys_are_stable_and_distinct() {
            let q = "SELECT ?s WHERE { ?s ?p ?o }";
            assert_eq!(
                build_search_cache_key(q, 100, true),
                build_search_cache_key(q, 100, true),
            );
            assert_ne!(
                build_search_cache_key(q, 100, true),
                build_search_cache_key(q, 200, true),
            );
            assert_ne!(
                build_search_cache_key(q, 100, true),
                lotus::state::build_export_cache_key(q),
            );
        }
    }
}

#[cfg(target_arch = "wasm32")]
// `unreachable_pub`-style narrowing requires `pub(crate)` for cross-module
// use; the nursery `redundant_pub_crate` suggestion (`pub`) would widen it.
#[allow(clippy::redundant_pub_crate)]
pub(crate) use cache_impl::{get_cached, store_cached};
