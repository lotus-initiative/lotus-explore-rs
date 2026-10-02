// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![doc = include_str!("../README.md")]
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
    FallbackService, Nomenclature, all_compounds_query, compounds_by_taxon_query,
    compounds_by_taxon_query_with, construct_from_select, counts_query, escape_sparql_string,
    escape_structure_literal, export_query, is_reference_lookup, limit_query,
    normalize_digits_expr, structure_search_query, structure_search_query_with,
    taxon_common_name_lookup_query, taxon_lookup_query, wdqs_fallback, with_filters,
};
