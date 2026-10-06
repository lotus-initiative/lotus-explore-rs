// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

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
