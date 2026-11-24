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
    let set = set_of(&["Alpha"]);
    cache.insert("one".to_owned(), Arc::clone(&set));

    assert!(cache.get("two").is_none(), "keys are the whole query");
    assert!(
        cache.any().is_some_and(|cached| Arc::ptr_eq(&cached, &set)),
        "any() still exposes the cached set to seed entity dictionaries across searches"
    );
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

#[test]
fn richest_set_is_retained_for_seeding_when_smaller_set_replaces_recent() {
    let mut cache = SetCache::new();
    let broad = set_of(&["Alpha", "Beta", "Gamma"]);
    let narrow = set_of(&["Delta"]);

    cache.insert("broad".to_owned(), Arc::clone(&broad));
    cache.insert("narrow".to_owned(), Arc::clone(&narrow));

    assert!(
        cache
            .get("narrow")
            .is_some_and(|s| Arc::ptr_eq(&s, &narrow)),
        "recent key resolves to narrow set"
    );
    assert!(
        cache.any().is_some_and(|s| Arc::ptr_eq(&s, &broad)),
        "any() retains the richest dictionary set so narrow searches do not evict broad seed dictionaries"
    );
}

#[test]
fn seed_reader_combines_richest_extra_and_recent_seed_sets() {
    let mut cache = SetCache::new();
    let plant = Arc::new(ColumnarResultSet::from_entries(&[CompoundEntry {
        compound_qid: Arc::from("Q10"),
        name: Arc::from("Plant Compound"),
        ..Default::default()
    }]));
    let fungi = Arc::new(ColumnarResultSet::from_entries(&[CompoundEntry {
        compound_qid: Arc::from("Q20"),
        name: Arc::from("Fungi Compound"),
        ..Default::default()
    }]));
    let extra = Arc::new(ColumnarResultSet::from_entries(&[CompoundEntry {
        compound_qid: Arc::from("Q30"),
        name: Arc::from("Bacteria Compound"),
        ..Default::default()
    }]));

    cache.insert("plant".to_owned(), plant);
    cache.insert("fungi".to_owned(), fungi);
    cache.store_extra_seed(extra);
    assert!(cache.has_broad_seed());

    let mut reader = lotus_query::CsvColumnarReader::new();
    reader
        .feed(b"compound,taxon,taxon_name,ref_qid\n10,1,A,\n20,2,B,\n30,3,C,\n")
        .expect("feed occurrences");
    cache.seed_reader(&mut reader);

    assert!(
        reader.compound_qids().next().is_none(),
        "all three compounds must be seeded from richest, recent seed_sets, and extra_seed"
    );
}
