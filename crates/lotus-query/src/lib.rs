// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

mod error;
mod export;
mod export_rows;
pub use export_rows::RowExporter;
mod parse;
mod query;

pub use error::ParseError;
pub use export::{ExportFormat, sanitize_download_filename};
pub use parse::CompoundMatch;
pub use parse::{
    CsvColumnarReader, CsvSplitter, ReferenceMatch, parse_compound_lookup_csv,
    parse_compounds_columnar, parse_compounds_csv, parse_compounds_csv_capped, parse_counts_csv,
    parse_reference_lookup_csv, parse_taxon_csv,
};
pub use query::{
    FallbackService, Nomenclature, all_compounds_including_untaxonomised_query,
    all_compounds_query, compound_alias_query, compound_by_qid_query, compound_inchikey_query,
    compound_label_query, compounds_by_taxon_query, compounds_by_taxon_query_with,
    construct_from_select, counts_query, escape_sparql_string, escape_structure_literal,
    exact_compound_query, export_query, is_reference_lookup, limit_query, normalize_digits_expr,
    reference_by_doi_query, reference_by_qid_query, structure_compound_lookup_query,
    structure_search_query, structure_search_query_with, taxon_common_name_lookup_query,
    taxon_lookup_query, wdqs_fallback, with_filters,
};
