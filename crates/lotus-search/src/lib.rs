// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
// Awaiting a retry backoff on the browser means awaiting a JS promise, and a JS
// promise is `!Send` -- it holds an `Rc`. There is no `Send`-preserving way to
// hand control back to the single-threaded runtime that owns it, so every future
// here is `!Send` when built for wasm. Native keeps the guarantee; see the
// workspace's `nursery` note on why the deny is worth keeping.
#![cfg_attr(target_arch = "wasm32", allow(clippy::future_not_send))]
// This crate dev-depends on itself to turn on `testing` for its own tests; see the
// comment on that line in `Cargo.toml`. Cargo cannot express a dev-dependency that
// only applies to this crate's integration tests, so the lib target links the
// self-dependency without naming it and `unused_crate_dependencies` reports it.
// The lint stays denied workspace-wide, where it still catches real dead
// dependencies in the other crates -- the same trade `apps/lotus-explore-rs`
// makes for its feature-gated `tower` test dependency.
#![allow(unused_crate_dependencies)]

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
    DEFAULT_ROW_LIMIT, ResolvedInputs, SearchError, StreamProgress, StructurePlan,
    StructureResolution, build_base_query, build_execution_query, build_execution_query_with,
    columnar_from_chunks, columnar_from_chunks_reporting, counts, is_qid, looks_like_inchikey,
    normalize_structure, resolve_inputs, resolve_reference, resolve_structure, resolve_taxon,
    search, search_columnar, standardize_taxon_name,
};

#[cfg(feature = "reqwest")]
pub mod reqwest_client;

#[cfg(feature = "testing")]
#[cfg_attr(docsrs, doc(cfg(feature = "testing")))]
pub mod testing;

pub use lotus_model::*;

/// `QLever`'s Wikidata endpoint. Faster than WDQS, and the default.
pub const QLEVER_WIKIDATA: &str = "https://qlever.dev/api/wikidata";

/// The Wikidata Query Service, used when `QLever` is unreachable.
pub const WDQS_WIKIDATA: &str = "https://query.wikidata.org/sparql";

/// WDQS's scholarly subgraph, which carries the reference properties P1476, P356
/// and P577 that the main endpoint serves poorly.
pub const WDQS_SCHOLARLY: &str = "https://query-scholarly.wikidata.org/sparql";
