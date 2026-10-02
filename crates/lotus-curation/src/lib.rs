// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
// Awaiting the search means awaiting a JS promise on the browser, and a JS
// promise is `!Send` -- it holds an `Rc`. There is no `Send`-preserving way to
// hand control back to the single-threaded runtime that owns it, so every future
// here is `!Send` when built for wasm. Native keeps the guarantee; see the
// workspace's `nursery` note on why the deny is worth keeping.
#![cfg_attr(target_arch = "wasm32", allow(clippy::future_not_send))]

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
