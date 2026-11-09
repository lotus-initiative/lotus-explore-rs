// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

mod bulk_export;
mod error;
mod export;
mod export_rows;
pub use export_rows::RowExporter;
mod parse;
mod query;

pub use bulk_export::{
    ARCHIVE_PAGE_SIZE, ARCHIVE_SMILES_QUERY, ARCHIVE_SMILES_WITHOUT_INCHIKEY_QUERY,
    COMPOUND_CID_QUERY, COMPOUND_CXSMILES_QUERY, COMPOUND_INCHIKEY_TAXON_QUERY,
    COMPOUND_LABEL_QUERY, COMPOUND_PARENT_QUERY, COMPOUND_SMARTS_QUERY,
    COMPOUND_SMILES_CAN_ALL_QUERY, COMPOUND_SMILES_CAN_QUERY, COMPOUND_SMILES_ISO_ALL_QUERY,
    COMPOUND_SMILES_ISO_QUERY, COMPOUND_TAXON_REFERENCE_QUERY, EXPORT_COMPOUNDS_QUERY,
    EXPORT_REFERENCES_QUERY, EXPORT_TAXA_QUERY, QLEVER_WIKIDATA, REFERENCE_DATE_QUERY,
    REFERENCE_DOI_COMPOUND_TAXON_QUERY, REFERENCE_DOI_QUERY, REFERENCE_JOURNAL_QUERY,
    REFERENCE_PMCID_QUERY, REFERENCE_PMID_COMPOUND_TAXON_QUERY, REFERENCE_PMID_QUERY,
    REFERENCE_TITLE_QUERY, TAXON_GBIF_QUERY, TAXON_NAME_QUERY, TAXON_NCBI_QUERY, TAXON_OTT_QUERY,
    TAXON_PARENT_QUERY, TAXON_RANK_QUERY, WIKIDATA_ENTITY_PREFIX, archive_page, extract_qid,
    looks_like_archive_inchikey,
};
pub use error::ParseError;
pub use export::{ExportFormat, sanitize_download_filename};
pub use parse::CompoundMatch;
pub use parse::{
    CsvColumnarReader, CsvSplitter, ReferenceMatch, parse_compound_lookup_csv,
    parse_compounds_columnar, parse_compounds_csv, parse_compounds_csv_capped, parse_counts_csv,
    parse_reference_lookup_csv, parse_taxon_csv,
};
pub use query::{
    DICTIONARY_VALUES_LIMIT, FallbackService, Nomenclature, SELECT_COLUMNS,
    all_compounds_including_untaxonomised_query, all_compounds_query, compound_alias_query,
    compound_by_qid_query, compound_dictionary_query, compound_inchikey_query,
    compound_label_query, compounds_by_taxon_query, compounds_by_taxon_query_with,
    compounds_by_taxon_query_with_resolved_closure, construct_from_select, counts_query,
    dictionary_queries, entity_dictionary_query, escape_sparql_string, escape_structure_literal,
    exact_compound_query, exact_compounds_query, export_query, is_reference_lookup, limit_query,
    normalize_digits_expr, occurrences_query, reference_by_doi_query, reference_by_qid_query,
    reference_dictionary_query, structure_compound_lookup_query, structure_search_query,
    structure_search_query_with, taxon_common_name_lookup_query, taxon_lookup_query, wdqs_fallback,
    with_filters,
};
