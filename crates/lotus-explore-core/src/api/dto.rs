// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Data Transfer Objects (DTOs) for the LOTUS API client.

use lotus_model::WIKIDATA_STATEMENT_BASE;
use lotus_model::{CompoundEntry, DatasetStats, ElementState, SearchCriteria, SmilesSearchType};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// How a structure is matched against the compound it names.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
enum ApiSmilesSearchType {
    /// The same molecule.
    Exact,
    /// A molecule containing the given one.
    Substructure,
    /// A molecule similar to the given one.
    Similarity,
}

impl From<SmilesSearchType> for ApiSmilesSearchType {
    fn from(value: SmilesSearchType) -> Self {
        match value {
            SmilesSearchType::Exact => Self::Exact,
            SmilesSearchType::Substructure => Self::Substructure,
            SmilesSearchType::Similarity => Self::Similarity,
        }
    }
}

/// What the server is told to do with an element.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
enum ApiElementState {
    /// The element may be present.
    Allowed,
    /// The element must be present.
    Required,
    /// The element must be absent.
    Excluded,
}

impl From<ElementState> for ApiElementState {
    fn from(value: ElementState) -> Self {
        match value {
            ElementState::Allowed => Self::Allowed,
            ElementState::Required => Self::Required,
            ElementState::Excluded => Self::Excluded,
        }
    }
}

/// The body of a `POST /v1/search`.
///
/// Built from a [`SearchCriteria`] rather than assembled by hand, so the
/// request always says the same thing the query does. A field is left out
/// rather than sent as a default, because the server treats "absent" and
/// "present but empty" differently for the year bounds.
#[derive(Debug, Serialize)]
pub struct SearchRequest {
    taxon: Option<String>,
    /// A Wikidata QID or a DOI. Resolved server-side, like `taxon`.
    reference: Option<String>,
    structure: Option<String>,
    structure_search: Option<ApiSmilesSearchType>,
    structure_threshold: Option<f64>,
    mass_min: Option<f64>,
    mass_max: Option<f64>,
    year_min: Option<u16>,
    year_max: Option<u16>,
    formula_exact: Option<String>,
    c_min: Option<u16>,
    c_max: Option<u16>,
    h_min: Option<u16>,
    h_max: Option<u16>,
    n_min: Option<u16>,
    n_max: Option<u16>,
    o_min: Option<u16>,
    o_max: Option<u16>,
    p_min: Option<u16>,
    p_max: Option<u16>,
    s_min: Option<u16>,
    s_max: Option<u16>,
    f_state: Option<ApiElementState>,
    cl_state: Option<ApiElementState>,
    br_state: Option<ApiElementState>,
    i_state: Option<ApiElementState>,
    limit: Option<usize>,
    include_counts: Option<bool>,
}

impl SearchRequest {
    /// The request that asks for the search the criteria describe.
    ///
    /// `year` is the current year, not a year read from a clock: the criteria
    /// know a publication year up to some bound, and whether that bound is
    /// still "the future" is a question about today. The caller supplies
    /// today.
    #[must_use]
    pub fn from_criteria(
        criteria: &SearchCriteria,
        limit: usize,
        include_counts: bool,
        year: u16,
    ) -> Self {
        let taxon = criteria.taxon.trim();
        let smiles = lotus_search::normalize_structure(&criteria.structure);
        let has_smiles = !smiles.is_empty();
        let formula_exact = criteria.formula_exact.trim();

        Self {
            taxon: (!taxon.is_empty()).then(|| taxon.to_string()),
            reference: {
                let reference = criteria.reference.trim();
                (!reference.is_empty()).then(|| reference.to_string())
            },
            structure: has_smiles.then_some(smiles),
            structure_search: has_smiles.then_some(criteria.structure_search.into()),
            structure_threshold: (criteria.structure_search == SmilesSearchType::Similarity
                && has_smiles)
                .then_some(criteria.structure_threshold),
            mass_min: criteria.has_mass_filter().then_some(criteria.mass_min),
            mass_max: criteria.has_mass_filter().then_some(criteria.mass_max),
            year_min: criteria.has_year_filter(year).then_some(criteria.year_min),
            year_max: criteria.has_year_filter(year).then_some(criteria.year_max),
            formula_exact: (!formula_exact.is_empty()).then(|| formula_exact.to_string()),
            c_min: criteria.formula_enabled.then_some(criteria.c_min),
            c_max: criteria.formula_enabled.then_some(criteria.c_max),
            h_min: criteria.formula_enabled.then_some(criteria.h_min),
            h_max: criteria.formula_enabled.then_some(criteria.h_max),
            n_min: criteria.formula_enabled.then_some(criteria.n_min),
            n_max: criteria.formula_enabled.then_some(criteria.n_max),
            o_min: criteria.formula_enabled.then_some(criteria.o_min),
            o_max: criteria.formula_enabled.then_some(criteria.o_max),
            p_min: criteria.formula_enabled.then_some(criteria.p_min),
            p_max: criteria.formula_enabled.then_some(criteria.p_max),
            s_min: criteria.formula_enabled.then_some(criteria.s_min),
            s_max: criteria.formula_enabled.then_some(criteria.s_max),
            f_state: criteria.formula_enabled.then_some(criteria.f_state.into()),
            cl_state: criteria.formula_enabled.then_some(criteria.cl_state.into()),
            br_state: criteria.formula_enabled.then_some(criteria.br_state.into()),
            i_state: criteria.formula_enabled.then_some(criteria.i_state.into()),
            limit: Some(limit),
            include_counts: Some(include_counts),
        }
    }
}

fn normalize_statement(value: Option<String>) -> Option<String> {
    let value = value?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(
        trimmed
            .strip_prefix(WIKIDATA_STATEMENT_BASE)
            .unwrap_or(trimmed)
            .to_string(),
    )
}

/// The body of a `POST /v1/search` answer.
#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    /// The QID the taxon name resolved to.
    pub resolved_taxon_qid: Option<String>,
    /// A message from the server about the search, if it had one.
    pub warning: Option<String>,
    /// The query the server actually ran.
    pub query: String,
    /// The rows it returned.
    pub rows: Vec<RowDto>,
    /// How many rows the whole result set has.
    ///
    /// Used only to decide whether the rows that arrived are the whole set: a
    /// partial page is declined, because the columnar store takes its counts from
    /// the rows it holds and a page would understate the total. The API's own
    /// `stats` block is not read -- the set computes the same five numbers, and
    /// exactly, from the rows themselves.
    pub total_matches: usize,
}

/// Where an export can be downloaded from.
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, Deserialize)]
// Field names mirror the `/v1` JSON wire format (`*_url`); renaming would
// break deserialization without per-field `serde(rename)` churn.
#[allow(clippy::struct_field_names)]
pub struct ExportUrlResponse {
    /// The uncompressed CSV.
    pub csv_url: String,
    /// The uncompressed JSON.
    pub json_url: String,
    /// The uncompressed RDF.
    pub rdf_url: String,
    /// The gzipped CSV, if the server made one.
    #[serde(default)]
    pub csv_gz_url: Option<String>,
    /// The gzipped JSON, if the server made one.
    #[serde(default)]
    pub json_gz_url: Option<String>,
    /// The gzipped RDF, if the server made one.
    #[serde(default)]
    pub rdf_gz_url: Option<String>,
}

/// How many of what the result set holds.
///
/// The counters the server reports, folded into the same shape the columnar
/// set computes for itself.
#[derive(Debug, Deserialize)]
// Field names mirror `DatasetStats` counters (`n_*`); renaming would break
// the serde DTO mapping without per-field `serde(rename)` churn.
#[allow(clippy::struct_field_names)]
pub struct SearchStats {
    /// How many distinct compounds.
    pub n_compounds: usize,
    /// How many distinct taxa.
    pub n_taxa: usize,
    /// How many distinct references.
    pub n_references: usize,
    /// How many entries, counting duplicates across taxa.
    #[serde(default)]
    pub n_entries: Option<usize>,
    /// How many distinct entries, when the server counted them.
    #[serde(default)]
    pub n_entries_unique: Option<usize>,
}

impl From<SearchStats> for DatasetStats {
    fn from(value: SearchStats) -> Self {
        let n_entries = value.n_entries.unwrap_or(0);
        Self {
            n_compounds: value.n_compounds,
            n_taxa: value.n_taxa,
            n_references: value.n_references,
            n_entries,
            n_entries_unique: value.n_entries_unique.unwrap_or(n_entries),
        }
    }
}

/// One row of a search answer.
#[derive(Debug, Deserialize)]
pub struct RowDto {
    /// The compound's QID.
    pub compound_qid: String,
    /// The compound's name.
    pub name: String,
    /// The compound's `InChIKey`, if it has one.
    pub inchikey: Option<String>,
    /// The compound's SMILES, if it has one.
    pub smiles: Option<String>,
    /// The compound's exact mass.
    pub mass: Option<f64>,
    /// The compound's molecular formula.
    pub formula: Option<String>,
    /// The reporting taxon's QID.
    pub taxon_qid: String,
    /// The reporting taxon's name.
    pub taxon_name: String,
    /// The reference's QID.
    pub reference_qid: String,
    /// The reference's title.
    pub ref_title: Option<String>,
    /// The reference's DOI.
    pub ref_doi: Option<String>,
    /// The reference's publication year.
    pub pub_year: Option<i16>,
    /// The Wikidata statement the entry came from.
    pub statement: Option<String>,
}

impl From<RowDto> for CompoundEntry {
    fn from(value: RowDto) -> Self {
        Self {
            compound_qid: Arc::<str>::from(value.compound_qid),
            name: Arc::<str>::from(value.name),
            inchikey: value.inchikey.map(Arc::<str>::from),
            smiles: value.smiles.map(Arc::<str>::from),
            mass: value.mass,
            formula: value.formula.map(Arc::<str>::from),
            taxon_qid: Arc::<str>::from(value.taxon_qid),
            taxon_name: Arc::<str>::from(value.taxon_name),
            reference_qid: Arc::<str>::from(value.reference_qid),
            reference_node: Arc::from(""),
            ref_title: value.ref_title.map(Arc::<str>::from),
            ref_doi: value.ref_doi.map(Arc::<str>::from),
            pub_year: value.pub_year,
            statement: normalize_statement(value.statement).map(Arc::<str>::from),
        }
    }
}

#[cfg(test)]
#[path = "dto/tests.rs"]
mod tests;
