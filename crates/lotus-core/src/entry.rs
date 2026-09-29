// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use std::sync::Arc;

/// One compound-found-in-taxon-cited-by-reference result row.
///
/// Every field is an `Arc<str>` so a row clones cheaply: the table renders and
/// re-sorts the same set many times, and a result set holds millions of rows.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct CompoundEntry {
    /// Wikidata QID of the compound.
    pub compound_qid: Arc<str>,
    /// Compound label, preferring the `mul` term over the `en` one.
    pub name: Arc<str>,
    /// `InChIKey` (P235).
    pub inchikey: Option<Arc<str>>,
    /// `SMILES`, preferring the isomeric form (P2017) over the
    /// connection-table one (P233).
    pub smiles: Option<Arc<str>>,
    /// Monoisotopic mass (P2067), in daltons.
    pub mass: Option<f64>,
    /// Molecular formula (P274), with subscript digits normalised to ASCII.
    pub formula: Option<Arc<str>>,
    /// Wikidata QID of the taxon. Empty when the compound has no occurrence.
    pub taxon_qid: Arc<str>,
    /// Taxon scientific name (P225). Empty when there is no taxon.
    pub taxon_name: Arc<str>,
    /// Wikidata QID of the cited reference. Empty when there is none.
    pub reference_qid: Arc<str>,
    /// Reference title (P1476).
    pub ref_title: Option<Arc<str>>,
    /// Reference DOI (P356), without the `doi.org` prefix.
    pub ref_doi: Option<Arc<str>>,
    /// Publication year, from P577.
    pub pub_year: Option<i16>,
    /// Wikidata statement QID backing the occurrence, without the
    /// `entity/statement/` prefix.
    pub statement: Option<Arc<str>>,
}

/// A result set. Cloning shares the rows rather than copying them.
pub type Rows = Arc<[CompoundEntry]>;

/// One hit from a taxon name lookup.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaxonMatch {
    /// Wikidata QID.
    pub qid: String,
    /// Scientific name (P225) or alias, as the endpoint reported it.
    pub name: String,
}
