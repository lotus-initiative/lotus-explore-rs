// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The LOTUS search use case.
//!
//! This is the layer the CLI, the web client and the API server all call: give
//! it a [`SearchCriteria`] and a taxon string, get back a [`SearchResult`] with
//! rows, a count, and notes about anything that had to be adjusted on the way.
//!
//! It is split from `lotus-query` on purpose. Query construction and CSV
//! parsing are pure and worth testing by string comparison; this layer owns the
//! decisions -- which endpoint to use, when to fall back, whether a taxon string
//! is a name or a `QID`, whether a structure search can run at all -- and those
//! decisions are worth testing through a recorded HTTP conversation instead.
//!
//! All IO goes through [`Http`], a two-method trait. [`reqwest_client`] satisfies
//! it for a real endpoint and is on by default; the tests satisfy it with a
//! script of canned answers. That is what lets the whole use case be tested
//! offline, including the fallback path, which is otherwise nearly impossible to
//! provoke.

#![warn(missing_docs)]

mod client;
mod error;
mod execute;
mod result;
mod search;

pub use client::{Http, HttpResponse, ResponseBody};
pub use error::{FetchError, ResponseFormat, is_retryable_status};
pub use execute::{Answer, Endpoint, Service, execute, execute_with_fallback, fetch_url};
pub use result::{SearchRequest, SearchResult, TaxonNote, TaxonResolution};
pub use search::{
    DEFAULT_ROW_LIMIT, SearchError, StructurePlan, build_base_query, build_execution_query, counts,
    is_qid, normalize_structure, resolve_taxon, search, standardize_taxon_name,
};

#[cfg(feature = "reqwest")]
pub mod reqwest_client;

pub use lotus_model::*;

/// `QLever`'s Wikidata endpoint. Faster than WDQS, and the default.
pub const QLEVER_WIKIDATA: &str = "https://qlever.dev/api/wikidata";

/// The Wikidata Query Service, used when `QLever` is unreachable.
pub const WDQS_WIKIDATA: &str = "https://query.wikidata.org/sparql";

/// WDQS's scholarly subgraph, which carries the reference properties P1476, P356
/// and P577 that the main endpoint serves poorly.
pub const WDQS_SCHOLARLY: &str = "https://query-scholarly.wikidata.org/sparql";
