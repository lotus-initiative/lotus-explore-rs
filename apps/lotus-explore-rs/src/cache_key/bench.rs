// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `cache_key`, in their own file.

use super::{build_export_cache_key, build_search_cache_key};
use sha2::{Digest, Sha256};
use std::time::Instant;

const SAMPLES: usize = 50;

/// A query per interesting size, from the real builders where one fits.
fn queries() -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = vec![
        (
            "tiny (a lookup, not a search)",
            lotus_query::taxon_lookup_query("Gentiana lutea"),
        ),
        (
            "everything, occurrence optional",
            lotus_query::all_compounds_including_untaxonomised_query(),
        ),
        (
            "one taxon (matrix cell 5)",
            lotus_query::compounds_by_taxon_query("Q16521"),
        ),
        (
            "taxon + nomenclature closure",
            lotus_query::compounds_by_taxon_query_with(
                "Q16521",
                &lotus_query::Nomenclature::ALL_ON,
            ),
        ),
    ];

    // A structure search carries the whole SERVICE block, so it is the longest
    // of the twelve and the one worth seeing at the top of the range.
    out.push((
        "structure search (longest of the twelve)",
        lotus_query::structure_search_query(
            "CC(=O)Oc1ccccc1C(=O)O",
            lotus_model::SmilesSearchType::Similarity,
            0.7,
            None,
        ),
    ));

    // And one far past anything the app builds, to show the curve is linear
    // rather than hiding a cliff at the sizes actually used.
    let base = lotus_query::compounds_by_taxon_query("Q16521");
    out.push(("synthetic 16x (past any real query)", base.repeat(16)));

    out
}

/// `(median, min, max)`. Iterator access rather than indexing, because the
/// workspace denies `clippy::indexing_slicing`.
fn median_ns(mut samples: Vec<u128>) -> (u128, u128, u128) {
    samples.sort_unstable();
    let mid = samples.len() / 2;
    (
        samples
            .get(mid)
            .copied()
            .expect("SAMPLES is a non-zero const"),
        *samples.first().expect("SAMPLES is a non-zero const"),
        *samples.last().expect("SAMPLES is a non-zero const"),
    )
}

#[test]
#[ignore = "a benchmark: prints its numbers, asserts nothing"]
fn bench_cache_key() {
    let cases = queries();

    println!("\ncache-key construction, {SAMPLES} samples, nanoseconds");
    println!(
        "{:<40} {:>7} {:>9} {:>9} {:>9} {:>9}",
        "query", "bytes", "export", "search", "sha256", "hex"
    );

    for (name, query) in &cases {
        // One untimed round each, so the first timed round is not paying for a
        // cold allocator.
        let _ = build_export_cache_key(query);
        let _ = build_search_cache_key(query, 500, true);

        let mut export = Vec::with_capacity(SAMPLES);
        let mut search = Vec::with_capacity(SAMPLES);
        let mut hash = Vec::with_capacity(SAMPLES);
        let mut hex = Vec::with_capacity(SAMPLES);
        let bytes = query.as_bytes();

        for _ in 0..SAMPLES {
            let start = Instant::now();
            let key = build_export_cache_key(query);
            export.push(start.elapsed().as_nanos());
            std::hint::black_box(key);

            let start = Instant::now();
            let key = build_search_cache_key(query, 500, true);
            search.push(start.elapsed().as_nanos());
            std::hint::black_box(key);

            // The two halves the key is built from, measured on their own so
            // the split is visible rather than inferred.
            let start = Instant::now();
            let digest = Sha256::digest(bytes);
            hash.push(start.elapsed().as_nanos());

            let start = Instant::now();
            let encoded = super::sha256_hex(digest);
            hex.push(start.elapsed().as_nanos());
            std::hint::black_box(encoded);
        }

        let (e, _, _) = median_ns(export);
        let (s, _, _) = median_ns(search);
        let (h, _, _) = median_ns(hash);
        let (x, _, _) = median_ns(hex);
        println!("{name:<40} {:>7} {e:>9} {s:>9} {h:>9} {x:>9}", query.len());
    }

    println!(
        "\n`export` and `search` include the hex encoding and the prefix `format!`.\n\
         A key is built once per request and once per export, so compare against\n\
         the network round trip it sits in front of rather than in isolation."
    );
}
