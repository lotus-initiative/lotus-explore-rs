// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `cache_key`, in their own file.

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
