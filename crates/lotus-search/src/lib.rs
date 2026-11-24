// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
// Awaiting a retry backoff on the browser means awaiting a JS promise, and a JS
// promise is `!Send` -- it holds an `Rc`. There is no `Send`-preserving way to
// hand control back to the single-threaded runtime that owns it, so every future
// here is `!Send` when built for wasm. Native keeps the guarantee; see the
// workspace's `nursery` note on why the deny is worth keeping.
#![cfg_attr(
    target_arch = "wasm32",
    allow(
        clippy::future_not_send,
        reason = "browser backoff promises hold Rc and are !Send"
    )
)]
// This crate dev-depends on itself to turn on `testing` for its own tests; see the
// comment on that line in `Cargo.toml`. Cargo cannot express a dev-dependency that
// only applies to this crate's integration tests, so the lib target links the
// self-dependency without naming it and `unused_crate_dependencies` reports it.
// The lint stays denied workspace-wide, where it still catches real dead
// dependencies in the other crates -- the same trade `apps/lotus-explore-rs`
// makes for its feature-gated `tower` test dependency.
#![allow(
    unused_crate_dependencies,
    reason = "self dev-dependency enables the testing feature for integration tests"
)]

mod client;
mod error;
mod execute;
mod result;
mod search;

pub use client::{BodyChunks, ChunkFuture, ChunkedBody, Http, HttpResponse, ResponseBody};
pub use error::{FetchError, ResponseFormat, TruncationReason, is_retryable_status};
pub use execute::{
    Answer, Endpoint, Service, StreamAnswer, execute, execute_streaming,
    execute_streaming_with_fallback, execute_with_fallback, fetch_url,
};
pub use result::{ColumnarSearchResult, SearchRequest, SearchResult, TaxonNote, TaxonResolution};
pub use search::{
    ResolvedInputs, SearchError, StreamProgress, StructureResolution, build_base_query,
    build_execution_query, build_execution_query_with, columnar_from_chunks,
    columnar_from_chunks_reporting, counts, enrich_columnar_reader, feed_chunks_reporting,
    resolve_inputs, resolve_reference, resolve_structure, resolve_taxon, search, search_columnar,
};

#[cfg(feature = "reqwest")]
pub mod reqwest_client;

#[cfg(feature = "testing")]
#[cfg_attr(docsrs, doc(cfg(feature = "testing")))]
pub mod testing;

/// How this client identifies itself to `QLever` (`api-user-agent`) and native
/// HTTP targets including Wikimedia's REST Gateway (`User-Agent`).
pub(crate) const CLIENT_ID: &str = concat!(
    "lotus-explore-rs/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/lotus-initiative/lotus-explore-rs)"
);
