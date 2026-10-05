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
    /// Wikidata QID of the reference node (`prov:wasDerivedFrom`), distinct from
    /// `reference_qid`, which is the publication it points at via `pr:P248`.
    pub reference_node: Arc<str>,
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

/// Which property a taxon name was matched on.
///
/// This is not a detail of the lookup: it decides whether the search gets a note
/// telling the reader that the name they typed is the discouraged kind. See
/// [`TaxonNameSource::Common`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub enum TaxonNameSource {
    /// The scientific name, `P225` — the name a taxonomic authority endorses and
    /// the one a publication is filed under.
    #[default]
    Scientific,
    /// The common name, `P1843` — what people call the organism in prose.
    ///
    /// Resolved deliberately: refusing it would send the reader to Wikidata for a
    /// lookup this tool already did. But it is not the name the compounds are filed
    /// under, and two organisms may share it. So it wins over a common-name
    /// reading of the input and always produces a notice saying what it resolved
    /// to.
    Common,
}

/// One hit from a taxon name lookup.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaxonMatch {
    /// Wikidata QID.
    pub qid: String,
    /// Scientific name (P225) or common name (P1843), as the endpoint reported it.
    pub name: String,
    /// Which of the two the name came from.
    pub source: TaxonNameSource,
}
