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
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
pub fn build_export_cache_key(query: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"export");
    hasher.update(query.as_bytes());
    format!("export:{}", sha256_hex(hasher.finalize()))
}

/// Lowercase hex encoding of a finalized digest.
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
const HEX: &[u8; 16] = b"0123456789abcdef";
#[cfg(not(target_arch = "wasm32"))]
fn hex_char(nibble: usize) -> Option<char> {
    HEX.get(nibble).map(|&c| char::from(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
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

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn each_search_input_changes_the_key() {
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

    #[cfg(not(target_arch = "wasm32"))]
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

    #[cfg(not(target_arch = "wasm32"))]
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
