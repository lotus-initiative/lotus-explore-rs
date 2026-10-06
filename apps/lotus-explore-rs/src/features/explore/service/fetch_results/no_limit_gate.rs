// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The interactive search must not put a row limit back on the query.
//!
//! A source-level check, because the file it guards is `wasm`-only. What it
//! pins is that `limit_query` is not reachable from the browser fetch path: a
//! `LIMIT` is appended server-side, so nothing downstream can undo it, and the
//! table's filters would go back to seeing only the rows inside it while the
//! toolbar reported the whole set's count.
//!
//! The tests look for the identifier *followed by a parenthesis*, which is a
//! call. Matching the bare name would also match this module's own prose, and a
//! gate that fails when someone documents the thing it forbids is a gate that
//! gets deleted.

/// The wasm fetch module, as text. Compiled here only to be read.
const WASM_FETCH: &str = include_str!("wasm.rs");

#[test]
fn the_fetch_path_never_limits_the_query() {
    assert!(
        !WASM_FETCH.contains("limit_query("),
        "a server-side LIMIT truncates the result before anything downstream \
         can see it, which is the bug this whole path exists to remove"
    );
    assert!(
        WASM_FETCH.contains("sparql_columnar"),
        "the browser fetches the whole set, streamed into a columnar store"
    );
}

#[test]
fn there_is_still_no_count_query() {
    assert!(
        !WASM_FETCH.contains("counts_query("),
        "the set carries its own exact counts, so a second query is pure cost"
    );
}
