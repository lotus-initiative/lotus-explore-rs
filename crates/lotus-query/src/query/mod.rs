// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Query construction.
//!
//! Every query is a `SELECT` over the LOTUS projection in Wikidata, shaped as nested
//! subqueries so `QLever` filters before it enriches. The nesting is load-bearing and
//! asserted in `tests/query_contract.rs`, which checks builders' behaviour rather than
//! their bytes: whitespace may change, structure may not.

mod builders;
mod builders_body;
mod compound_names;
mod dictionary;
mod filters;
mod lookups_compound;
mod lookups_taxon;
mod nomenclature;
mod prefixes;
mod rewrite;
mod structure;

#[cfg(test)]
mod tests;

pub use builders::{
    all_compounds_including_untaxonomised_query, all_compounds_query, base_query_for_criteria,
    compounds_by_taxon_query, compounds_by_taxon_query_with,
    compounds_by_taxon_query_with_resolved_closure, exact_compound_query, exact_compounds_query,
};
pub use dictionary::{
    DICTIONARY_VALUES_LIMIT, compound_dictionary_query, counts_query, dictionary_queries,
    entity_dictionary_query, occurrences_query, reference_dictionary_query,
};
pub use filters::{limit_query, with_filters};
pub use lookups_compound::{
    compound_alias_query, compound_by_qid_query, compound_inchikey_query, compound_label_query,
    reference_by_doi_query, reference_by_qid_query, structure_compound_lookup_query,
};
pub use lookups_taxon::{
    has_data_row, taxon_common_name_lookup_query, taxon_has_compounds_query, taxon_lookup_query,
};
pub use nomenclature::Nomenclature;
pub use prefixes::SELECT_COLUMNS;
pub use rewrite::{
    FallbackService, construct_from_select, escape_sparql_string, escape_structure_literal,
    export_query, is_reference_lookup, normalize_digits_expr, wdqs_fallback,
};
pub use structure::{structure_search_query, structure_search_query_with};

#[cfg(test)]
use builders_body::{all_required_compounds_query_body, optional_compounds_query_body};
#[cfg(test)]
use compound_names::tagged_name_values;
