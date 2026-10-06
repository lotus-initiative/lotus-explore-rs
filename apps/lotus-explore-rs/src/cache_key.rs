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
#[path = "cache_key/tests.rs"]
mod tests;

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
#[path = "cache_key/bench.rs"]
mod bench;
