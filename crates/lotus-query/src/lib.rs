// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! SPARQL query construction and result parsing for LOTUS.
//!
//! Two halves, and neither one needs HTTP, an async runtime or a clock:
//!
//! - The `query` module builds query strings from a
//!   [`SearchCriteria`](lotus_model::SearchCriteria). Pure string manipulation
//!   over the vocabulary in [`lotus_model`].
//! - The `parse` module turns a `text/csv` payload into [`lotus_model`] types.
//!
//! Running the query is somebody else's job: see the `lotus-search` crate,
//! which drives this one against a real endpoint. Keeping that out is what
//! makes the builders testable by string comparison and the parsers testable
//! against recorded fixtures.

#![warn(missing_docs)]

mod error;
mod export;
mod parse;
mod query;

pub use error::ParseError;
pub use export::{ExportFormat, sanitize_download_filename};
pub use parse::{
    parse_compounds_csv, parse_compounds_csv_capped, parse_compounds_stream, parse_counts_csv,
    parse_taxon_csv,
};
pub use query::{
    FallbackService, all_compounds_query, compounds_by_taxon_query, construct_from_select,
    counts_query, escape_sparql_string, escape_structure_literal, export_query,
    is_reference_lookup, limit_query, normalize_digits_expr, structure_search_query,
    taxon_lookup_query, wdqs_fallback, with_filters,
};
