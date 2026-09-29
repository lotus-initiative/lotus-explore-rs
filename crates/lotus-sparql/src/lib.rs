// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! SPARQL query construction and result parsing for LOTUS.
//!
//! Two halves, and neither one has to know about HTTP to be used:
//!
//! - [`query`] builds query strings. Pure string manipulation.
//! - [`parse`] turns a `text/csv` payload into [`lotus_core`] types.
//!
//! Talking to an endpoint goes through [`Http`], a two-method trait. A caller
//! can satisfy it with `reqwest` (the [`reqwest_client`] module, on by default),
//! with a recorded fixture, or with whatever HTTP the platform already has —
//! which is what makes the web app, the CLI and the tests share this crate
//! unchanged.

#![warn(missing_docs)]

mod client;
mod error;
mod execute;
mod parse;
mod query;
mod result;
mod search;

pub use client::{Http, HttpResponse, ResponseBody};
pub use error::{FetchError, ResponseFormat, is_retryable_status};
pub use execute::{Answer, Endpoint, execute, execute_with_fallback, fetch_url};
pub use parse::{
    parse_compounds_csv, parse_compounds_csv_capped, parse_compounds_stream, parse_counts_csv,
    parse_taxon_csv,
};
pub use query::{
    all_compounds_query, compounds_by_taxon_query, construct_from_select, counts_query,
    escape_sparql_string, escape_structure_literal, export_query, is_reference_lookup, limit_query,
    normalize_digits_expr, structure_search_query, taxon_lookup_query, wdqs_fallback, with_filters,
};
pub use result::{SearchRequest, SearchResult, TaxonNote, TaxonResolution};
pub use search::{
    DEFAULT_ROW_LIMIT, SearchError, StructurePlan, build_base_query, build_execution_query, counts,
    is_qid, normalize_structure, resolve_taxon, search, standardize_taxon_name,
};

#[cfg(feature = "reqwest")]
pub mod reqwest_client;

pub use lotus_core::*;

/// `QLever`'s Wikidata endpoint. Faster than WDQS, and the default.
pub const QLEVER_WIKIDATA: &str = "https://qlever.dev/api/wikidata";

/// The Wikidata Query Service, used when `QLever` is unreachable.
pub const WDQS_WIKIDATA: &str = "https://query.wikidata.org/sparql";

/// WDQS's scholarly subgraph, which carries the reference properties P1476, P356
/// and P577 that the main endpoint serves poorly.
pub const WDQS_SCHOLARLY: &str = "https://query-scholarly.wikidata.org/sparql";
