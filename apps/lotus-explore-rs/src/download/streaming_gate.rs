// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! The download must not go back to buffering the whole export.
//!
//! A source-level check rather than a behavioural one, because the code it
//! guards is `wasm`-only and cannot be exercised by a native test run. What it
//! pins is the *call*: the WDQS path has to read the body through
//! `execute_sparql_chunks_at`, which streams, and must not reach for
//! `execute_sparql_format_at`, which returns the entire export as one `String`.
//!
//! That difference is invisible until it is fatal. A two-million-row export is
//! roughly 600 MB of CSV; decoded into a `String` in a tab budgeted for 200 MB
//! it is a killed tab with no error, and the download button simply does
//! nothing on a large result.
//!
//! The identifier is matched *with* its parenthesis, so a comment explaining
//! why the buffering call is gone does not trip the gate.

/// The wasm download module, as text. Compiled here only to be read.
const WASM_DOWNLOAD: &str = include_str!("wasm.rs");

#[test]
fn the_wdqs_download_streams_rather_than_buffering() {
    assert!(
        WASM_DOWNLOAD.contains("execute_sparql_chunks_at"),
        "the WDQS download should read the body through the chunked reader"
    );
    assert!(
        !WASM_DOWNLOAD.contains("execute_sparql_format_at("),
        "the WDQS download must not buffer the whole export into one String: \
         that is what this path was rewritten to stop doing"
    );
}

#[test]
fn a_streamed_export_is_handed_over_as_its_parts() {
    assert!(
        WASM_DOWNLOAD.contains("download_byte_chunks_as_blob"),
        "the collected chunks are what gets saved, not a decoded string"
    );
}
