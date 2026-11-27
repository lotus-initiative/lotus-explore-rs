// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! SHA-256 cache keys for the server's search and export caches.

use sha2::{Digest, Sha256};

/// The key for a search response, on the server's side.
///
/// The SPARQL is hashed rather than the filter values, so two searches that
/// build the same query share a cache entry. The limit and the counts flag are
/// mixed in because they change what the endpoint is asked to send.
#[must_use]
pub fn build_search_cache_key(query: &str, limit: usize, include_counts: bool) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"search");
    hasher.update(limit.to_le_bytes());
    hasher.update([u8::from(include_counts)]);
    hasher.update(query.as_bytes());
    prefixed_sha256_hex("search:", hasher.finalize())
}

/// The key for an export.
///
/// The format is left out: it is cheap to re-derive from the query, whereas the
/// result of running it is the expensive thing being cached.
#[must_use]
pub fn build_export_cache_key(query: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"export");
    hasher.update(query.as_bytes());
    prefixed_sha256_hex("export:", hasher.finalize())
}

/// Lowercase hex encoding of a finalized digest.
#[cfg(test)]
fn sha256_hex(bytes: impl AsRef<[u8]>) -> String {
    prefixed_sha256_hex("", bytes)
}

/// Lowercase hex encoding of a finalized digest, preceded by `prefix` in one allocation.
fn prefixed_sha256_hex(prefix: &str, bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut out = String::with_capacity(prefix.len() + bytes.len() * 2);
    out.push_str(prefix);
    lotus_model::push_hex_lower(&mut out, bytes);
    out
}

#[cfg(test)]
#[path = "cache_key/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "cache_key/bench.rs"]
mod bench;
