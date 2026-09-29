// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The LOTUS curation vocabulary.
//!
//! Curation means: a chemist has a list of compounds and organisms, and some of
//! those are in Wikidata and some are not. The job is to say which is which and
//! to write the statements that would fix the ones that are not.
//!
//! Everything here is pure — the types a row and a result have, the Wikidata
//! item identifiers they are written against, and how a batch of statements is
//! assembled for a curator to submit. Nothing here talks to Wikidata or to
//! `RDKit`. The web client and the CLI both curate, and they used to each carry
//! their own copy of these types, which is how two implementations of "what a
//! curation row is" ended up in the same repository.

#![warn(missing_docs)]

mod constants;
mod input;
mod internal;
mod knowledge;
mod quickstatements;
mod structure;
mod types;
mod wikidata_query;

/// The natural-products API, which converts a structure into the `InChIKey` that
/// Wikidata is matched on.
///
/// A plain URL, available on every target. The browser build converts through
/// `RDKit` compiled to WebAssembly instead, but the URL being present on wasm
/// costs nothing and gating it meant this crate could not be built there at all.
pub use constants::NATPROD_API_BASE;
pub use constants::{
    CURATION_SPARQL_PREFIXES, WD_CHEMICAL_COMPOUND_QID, WD_OCCURS_IN_TAXON_PROP,
    WD_STEREOISOMER_GROUP_QID, WD_TAXON_QID, WD_TYPE_CHEMICAL_ENTITY_QID,
};
pub use input::{parse_tsv, row_uniqueness_key};
pub use internal::{DependencyResolution, MassResolution, WikidataCompound};
pub use knowledge::{
    StructureKey, WikidataLookup, creation_statements, look_up, statements_for,
    taxon_dependency_statements, to_result_row,
};
pub use quickstatements::{build_quickstatements_bundle, escape_quickstatements};
pub use structure::{ConvertedStructure, convert_structure, convert_structures};
pub use types::{
    CurationError, CurationErrorKind, CurationInputRow, CurationResultRow, CurationStatus,
    QuickStatementsBundle,
};
pub use wikidata_query::{
    compound_by_inchikey_query, create_compound_statements, escape_sparql_string,
    has_occurrence_query, is_binomial, property, qid_from_uri, reference_by_doi_query,
    taxon_by_name_query,
};
