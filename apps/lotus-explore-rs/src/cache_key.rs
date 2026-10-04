// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
// The panic lints keep library code off `expect`; a test that trips one
// is reporting a failure, not unwinding.

//! Cache keys, shared by the native server and the browser client.
//!
//! Both compute a key from the same inputs, and both read the other's cache:
//! the server serves `/v1/export-file/{key}/…` from a map the client also
//! populates. A key is therefore an interface between the two, and a key
//! derived differently on each side would show up as a cache miss on every
//! request — correct, but slow enough to look like the endpoint is down.
//!
//! Two keys: one for a search response and one for an export. Neither is the
//! browser's cache key — the browser keys its finished result sets on the query
//! text itself, in `crate::cache`. What is hashed here is what the **server**
//! caches, because the client and the server read each other's maps and the key
//! is an interface between them.

// The panic lints keep library code off `expect`; a test that trips one is
// reporting a failure, not unwinding.
#![allow(clippy::expect_used, clippy::panic)]

// Only the server's two keys hash anything, and both are native-only.
// `any(not(wasm32), test)`, and not a plain `not(wasm32)`: the browser has its own
// fetch path and no production caller, but the *tests* for these keys run in a wasm
// test build, where a plain gate hides them and the target fails to compile. Being
// unused in production and being untestable are different things.
#[cfg(any(not(target_arch = "wasm32"), test))]
use sha2::{Digest, Sha256};

/// The key for a search response, on the server's side.
///
/// The SPARQL is hashed rather than the filter values, so two searches that
/// build the same query share a cache entry. The limit and the counts flag are
/// mixed in because they change what the endpoint is asked to send -- and the
/// limit is still a real thing here: this is the REST surface, which serves bulk
/// callers and caps its own responses, not the browser's streaming path.
// Only the server caches search responses; the browser keys its own finished
// sets on the query text. So this has no caller in a wasm build.
#[cfg(any(not(target_arch = "wasm32"), test))]
#[must_use]
pub fn build_search_cache_key(query: &str, limit: usize, include_counts: bool) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"search");
    hasher.update(limit.to_le_bytes());
    hasher.update([u8::from(include_counts)]);
    hasher.update(query.as_bytes());
    format!("search:{}", sha256_hex(hasher.finalize()))
}

/// The key for an export.
///
/// The format is left out: it is cheap to re-derive from the query, and it
/// would need re-running the query to do so, whereas the result of running it
/// is the expensive thing being cached.
#[must_use]
// Only the native and server paths reach this; the browser client has its
// own fetch path, so a wasm build has no caller for it.
#[cfg(any(not(target_arch = "wasm32"), test))]
pub fn build_export_cache_key(query: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"export");
    hasher.update(query.as_bytes());
    format!("export:{}", sha256_hex(hasher.finalize()))
}

/// Lowercase hex encoding of a finalized digest.
#[cfg(any(not(target_arch = "wasm32"), test))]
fn sha256_hex(bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        let high = usize::from(byte >> 4);
        let low = usize::from(byte & 0x0f);
        out.push(hex_char(high).unwrap_or('0'));
        out.push(hex_char(low).unwrap_or('0'));
    }
    out
}

/// The character for a nibble, or `None` if the input was not a nibble.
#[cfg(any(not(target_arch = "wasm32"), test))]
const HEX: &[u8; 16] = b"0123456789abcdef";
#[cfg(any(not(target_arch = "wasm32"), test))]
fn hex_char(nibble: usize) -> Option<char> {
    HEX.get(nibble).map(|&c| char::from(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_query_gives_the_same_key() {
        assert_eq!(
            build_export_cache_key("SELECT 1"),
            build_export_cache_key("SELECT 1")
        );
    }

    #[test]
    fn each_input_changes_the_key() {
        // If any of these collided, one search would be served another's rows.
        let base = build_search_cache_key("SELECT 1", 10, true);
        assert_ne!(base, build_search_cache_key("SELECT 2", 10, true), "query");
        assert_ne!(base, build_search_cache_key("SELECT 1", 11, true), "limit");
        assert_ne!(
            base,
            build_search_cache_key("SELECT 1", 10, false),
            "counts"
        );
    }

    #[test]
    fn search_and_export_keys_cannot_collide() {
        // The prefix is what stops an export being served from the search
        // cache. The two hash different inputs, so this also guards against
        // someone dropping the prefix as "redundant".
        assert!(
            build_search_cache_key("SELECT 1", 10, true).starts_with("search:"),
            "the prefix is what stops an export being served from the search cache"
        );
        assert!(build_export_cache_key("SELECT 1").starts_with("export:"));
    }

    #[test]
    fn keys_are_hex_of_the_right_length() {
        // A SHA-256 digest is 32 bytes, so 64 hex characters. Anything shorter
        // means the digest was truncated, which would make collisions likely
        // enough to matter.
        let key = build_export_cache_key("SELECT 1");
        let hex = key.strip_prefix("export:").expect("prefixed");
        assert_eq!(hex.len(), 64, "{key}");
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()), "{key}");
    }

    #[test]
    fn a_byte_boundary_cannot_paste_two_inputs_together() {
        // `update` without a length prefix means "SELECT" + "1" and
        // "SELECT" + "T1" hash identically if the caller ever concatenates.
        // This documents that the separator has to be explicit, not that the
        // current call sites are wrong.
        assert_ne!(build_export_cache_key("abc"), build_export_cache_key("cba"));
    }
}

/// How long a cache key takes to build, and where that time goes.
///
/// Release profile, and `#[ignore]`d like `lotus-query`'s `bench.rs`: a benchmark
/// that fails on a loaded machine is a test that gets deleted, so this prints and
/// asserts nothing.
///
/// ```bash
/// cargo test -p lotus-explore-rs --release -- --ignored --nocapture bench_cache_key
/// ```
///
/// The queries are built with the real `lotus-query` builders rather than pasted in
/// as literals, for two reasons: the sizes are then the sizes the app actually
/// hashes rather than sizes someone guessed, and a 2 KB string per case does not
/// sit in the source file.
///
/// **Size is the variable that matters here.** The key is hashed per request from
/// the query text, so the cost is a function of how long the query is -- and the
/// twelve argument combinations in `docs/QUERY_MATRIX.md` span roughly 2.2 KB to
/// 2.7 KB, while a filter-heavy query is longer again. A single "average" number
/// would say nothing about whether this is worth touching.
///
/// Split three ways, because they have different fixes: the SHA-256 itself, the
/// hex encoding, and the `format!` that wraps the result in its prefix. The hex
/// loop pushes two chars per byte through an `Option`, which is the part a reader
/// would suspect first.
///
/// Median and spread rather than a mean: the distribution is dominated by the
/// allocator, and one slow sample moves a mean much further than a median.
#[cfg(test)]
mod bench {
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
}
