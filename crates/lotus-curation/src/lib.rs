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
mod quickstatements;
mod types;

/// The natural-products API, which the server build uses to canonicalise a
/// structure. Not compiled for wasm, where there is nothing to call it from.
#[cfg(not(target_arch = "wasm32"))]
pub use constants::NATPROD_API_BASE;
pub use constants::{
    CURATION_SPARQL_PREFIXES, WD_CHEMICAL_COMPOUND_QID, WD_OCCURS_IN_TAXON_PROP,
    WD_STEREOISOMER_GROUP_QID, WD_TAXON_QID, WD_TYPE_CHEMICAL_ENTITY_QID,
};
pub use input::{parse_tsv, row_uniqueness_key};
pub use internal::{DependencyResolution, MassResolution, WikidataCompound};
pub use quickstatements::build_quickstatements_bundle;
pub use types::{
    CurationError, CurationErrorKind, CurationInputRow, CurationResultRow, CurationStatus,
    QuickStatementsBundle,
};
