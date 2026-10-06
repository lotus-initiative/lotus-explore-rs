// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Data Transfer Objects (DTOs) for the LOTUS API client.

use lotus_model::WIKIDATA_STATEMENT_BASE;
use lotus_model::{CompoundEntry, DatasetStats, ElementState, SearchCriteria, SmilesSearchType};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
enum ApiSmilesSearchType {
    Exact,
    Substructure,
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

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
enum ApiElementState {
    Allowed,
    Required,
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
    pub fn from_criteria(criteria: &SearchCriteria, limit: usize, include_counts: bool) -> Self {
        let taxon = criteria.taxon.trim();
        let smiles = normalize_structure_for_api(&criteria.structure);
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
            year_min: criteria
                .has_year_filter(crate::clock::current_year())
                .then_some(criteria.year_min),
            year_max: criteria
                .has_year_filter(crate::clock::current_year())
                .then_some(criteria.year_max),
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

fn normalize_structure_for_api(value: &str) -> String {
    // Normalize CRLF → LF first, then bare CR → LF, to avoid double-converting.
    let normalized = if value.contains('\r') {
        value.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        value.to_string()
    };
    match lotus_model::classify_structure(&normalized) {
        lotus_model::StructureKind::MolfileV2000 | lotus_model::StructureKind::MolfileV3000 => {
            normalized
        }
        _ => normalized.trim().to_string(),
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

#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    pub resolved_taxon_qid: Option<String>,
    pub warning: Option<String>,
    pub query: String,
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

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, Deserialize)]
// Field names mirror the `/v1` JSON wire format (`*_url`); renaming would
// break deserialization without per-field `serde(rename)` churn.
#[allow(clippy::struct_field_names)]
pub struct ExportUrlResponse {
    pub csv_url: String,
    pub json_url: String,
    pub rdf_url: String,
    #[serde(default)]
    pub csv_gz_url: Option<String>,
    #[serde(default)]
    pub json_gz_url: Option<String>,
    #[serde(default)]
    pub rdf_gz_url: Option<String>,
}

#[derive(Debug, Deserialize)]
// Field names mirror `DatasetStats` counters (`n_*`); renaming would break
// the serde DTO mapping without per-field `serde(rename)` churn.
#[allow(clippy::struct_field_names)]
pub struct SearchStats {
    pub n_compounds: usize,
    pub n_taxa: usize,
    pub n_references: usize,
    #[serde(default)]
    pub n_entries: Option<usize>,
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

#[derive(Debug, Deserialize)]
pub struct RowDto {
    pub compound_qid: String,
    pub name: String,
    pub inchikey: Option<String>,
    pub smiles: Option<String>,
    pub mass: Option<f64>,
    pub formula: Option<String>,
    pub taxon_qid: String,
    pub taxon_name: String,
    pub reference_qid: String,
    pub ref_title: Option<String>,
    pub ref_doi: Option<String>,
    pub pub_year: Option<i16>,
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
