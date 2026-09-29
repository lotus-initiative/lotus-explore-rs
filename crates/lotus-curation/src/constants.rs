// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! The Wikidata items and properties curation writes against.
//!
//! These are identifiers, not tunables. Changing one changes what the generated
//! statements mean, so they are named constants in one file rather than literals
//! repeated across the query builders.

/// The natural-products API, which converts a structure into an `InChIKey`.
///
/// A plain URL, so it is available on every target. The browser build converts
/// through `RDKit` in WebAssembly instead and never asks for this, but the
/// constant existing on wasm costs nothing and gating it kept this crate from
/// being built there at all.
pub const NATPROD_API_BASE: &str = "https://api.naturalproducts.net/latest";

/// The `PREFIX` block every curation query starts with.
///
/// Templates embed it as a `{CURATION_SPARQL_PREFIXES}` placeholder and
/// substitute, so the prefixes stay in one place rather than being pasted into
/// every query.
pub const CURATION_SPARQL_PREFIXES: &str = "\
PREFIX wd: <http://www.wikidata.org/entity/>\n\
PREFIX wdt: <http://www.wikidata.org/prop/direct/>\n\
PREFIX p: <http://www.wikidata.org/prop/>\n\
PREFIX ps: <http://www.wikidata.org/prop/statement/>\n\
PREFIX prov: <http://www.w3.org/ns/prov#>\n\
PREFIX pr: <http://www.wikidata.org/prop/reference/>\n\
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>";

/// The Wikidata class a curated compound is an instance of.
pub const WD_CHEMICAL_COMPOUND_QID: &str = "Q11173";
/// The Wikidata class for the type of a chemical entity.
pub const WD_TYPE_CHEMICAL_ENTITY_QID: &str = "Q113145171";
/// The Wikidata class for the group of stereoisomers a structure belongs to.
pub const WD_STEREOISOMER_GROUP_QID: &str = "Q59199015";
/// The Wikidata property linking a compound to an organism it occurs in.
pub const WD_OCCURS_IN_TAXON_PROP: &str = "P703";
/// The QID of the taxon used in the examples.
pub const WD_TAXON_QID: &str = "Q16521";
