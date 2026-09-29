// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! In-process taxon name → QID cache.

use std::cell::RefCell;
use std::collections::HashMap;

type TaxonCache = HashMap<String, String>;

thread_local! {
    static CACHE: RefCell<TaxonCache> = RefCell::new(HashMap::new());
}

/// Returns the cached QID for the given taxon `name`, or `None` if not cached.
pub fn lookup(name: &str) -> Option<String> {
    let key = name.trim().to_lowercase();
    if key.is_empty() {
        return None;
    }
    CACHE.with(|cache| cache.borrow().get(&key).cloned())
}

/// Stores `qid` in the cache under the normalised form of `name`.
pub fn store(name: &str, qid: &str) {
    let key = name.trim().to_lowercase();
    if key.is_empty() || qid.trim().is_empty() {
        return;
    }
    CACHE.with(|cache| {
        cache.borrow_mut().insert(key, qid.into());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_returns_none_for_empty_key() {
        assert!(lookup("").is_none());
        assert!(lookup("   ").is_none());
    }

    #[test]
    fn store_and_lookup_roundtrip() {
        store("Gentiana lutea", "Q2598745");
        let result = lookup("gentiana lutea");
        assert_eq!(result.as_deref(), Some("Q2598745"));
    }

    #[test]
    fn store_ignores_empty_qid() {
        store("Somespecies", "");
        assert!(lookup("somespecies").is_none());
    }
}
