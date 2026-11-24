// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The queries curation asks Wikidata.
//!
//! Pure string construction, so the queries are testable by comparison and the
//! web client and the CLI cannot drift apart on what a "compound" means.

mod compound;
mod occurrence;
mod reference;
mod statements;
mod taxon;
mod taxon_name;

#[cfg(test)]
use crate::{WD_CHEMICAL_COMPOUND_QID, WD_TYPE_CHEMICAL_ENTITY_QID};

#[cfg(test)]
mod tests;

pub use compound::{build_compound_lookup_query, compound_by_inchikey_query};
pub use occurrence::{
    build_occurrence_query, build_occurrence_with_ref_query, has_occurrence_query,
    has_occurrence_with_ref_query,
};
pub use reference::{build_reference_lookup_query, reference_by_doi_query};
pub use statements::{
    create_compound_statements, escape_sparql_string, property, qid_from_uri, qid_number,
};
pub use taxon::{
    build_single_taxon_lookup_query, build_taxon_lookup_query, normalize_taxon_lookup,
    taxon_by_name_query, taxon_name_candidates,
};
pub use taxon_name::{extract_canonical_taxon_name, is_binomial};
