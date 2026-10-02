// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use lotus_model::{CompoundEntry, DatasetStats, SmilesSearchType};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    pub(crate) status: &'static str,
    pub(crate) uptime_secs: u64,
    pub(crate) search_cache_hits: u64,
    pub(crate) search_cache_misses: u64,
    pub(crate) search_inflight_waits: u64,
    pub(crate) search_upstream_hits: u64,
    pub(crate) export_cache_hits: u64,
    pub(crate) export_cache_misses: u64,
    pub(crate) export_inflight_waits: u64,
    pub(crate) export_upstream_hits: u64,
    pub(crate) overload_rejections: u64,
    pub(crate) request_timeouts: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ApiSmilesSearchType {
    Substructure,
    Similarity,
}

impl From<ApiSmilesSearchType> for SmilesSearchType {
    fn from(value: ApiSmilesSearchType) -> Self {
        match value {
            ApiSmilesSearchType::Substructure => Self::Substructure,
            ApiSmilesSearchType::Similarity => Self::Similarity,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ApiElementState {
    Allowed,
    Required,
    Excluded,
}

impl From<ApiElementState> for lotus_model::ElementState {
    fn from(value: ApiElementState) -> Self {
        match value {
            ApiElementState::Allowed => Self::Allowed,
            ApiElementState::Required => Self::Required,
            ApiElementState::Excluded => Self::Excluded,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SearchRequest {
    pub(crate) taxon: Option<String>,
    /// Whether the taxon filter also collects the compounds recorded against
    /// the taxon's other names.
    ///
    /// One field per nomenclatural relationship, because they answer different
    /// questions — *accepted name vs. synonym* is a taxonomic judgement, while
    /// *old vs. new* is a nomenclatural fact, and Wikidata stores them under
    /// four separate property pairs. See `docs/TAXON-SEARCH.md`.
    ///
    /// Each absent field is left at the model default rather than reconciled
    /// against it, so a caller that omits them all gets the same answer as the
    /// UI's default checkboxes.
    pub(crate) taxon_accepted_synonyms: Option<bool>,
    pub(crate) taxon_basionyms: Option<bool>,
    pub(crate) taxon_protonyms: Option<bool>,
    pub(crate) taxon_replacements: Option<bool>,
    pub(crate) smiles: Option<String>,
    pub(crate) smiles_search_type: Option<ApiSmilesSearchType>,
    /// Tanimoto cutoff for a similarity structure search. Read under the name it
    /// now has; the old `smiles_threshold` is still accepted, because links
    /// naming it are already in people's clipboard histories.
    #[serde(alias = "smiles_threshold")]
    pub(crate) similarity_threshold: Option<f64>,
    pub(crate) mass_min: Option<f64>,
    pub(crate) mass_max: Option<f64>,
    pub(crate) year_min: Option<u16>,
    pub(crate) year_max: Option<u16>,
    pub(crate) formula_exact: Option<String>,
    pub(crate) c_min: Option<u16>,
    pub(crate) c_max: Option<u16>,
    pub(crate) h_min: Option<u16>,
    pub(crate) h_max: Option<u16>,
    pub(crate) n_min: Option<u16>,
    pub(crate) n_max: Option<u16>,
    pub(crate) o_min: Option<u16>,
    pub(crate) o_max: Option<u16>,
    pub(crate) p_min: Option<u16>,
    pub(crate) p_max: Option<u16>,
    pub(crate) s_min: Option<u16>,
    pub(crate) s_max: Option<u16>,
    pub(crate) f_state: Option<ApiElementState>,
    pub(crate) cl_state: Option<ApiElementState>,
    pub(crate) br_state: Option<ApiElementState>,
    pub(crate) i_state: Option<ApiElementState>,
    pub(crate) limit: Option<usize>,
    pub(crate) include_counts: Option<bool>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SearchStats {
    #[serde(rename = "n_compounds")]
    pub(crate) compounds: usize,
    #[serde(rename = "n_taxa")]
    pub(crate) taxa: usize,
    #[serde(rename = "n_references")]
    pub(crate) references: usize,
    #[serde(rename = "n_entries")]
    pub(crate) entries: usize,
    #[serde(rename = "n_entries_unique")]
    pub(crate) unique_entries: usize,
}

impl From<DatasetStats> for SearchStats {
    fn from(value: DatasetStats) -> Self {
        Self {
            compounds: value.n_compounds,
            taxa: value.n_taxa,
            references: value.n_references,
            entries: value.n_entries,
            unique_entries: value.n_entries_unique,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RowDto {
    pub(crate) compound_qid: String,
    pub(crate) name: String,
    pub(crate) inchikey: Option<String>,
    pub(crate) smiles: Option<String>,
    pub(crate) mass: Option<f64>,
    pub(crate) formula: Option<String>,
    pub(crate) taxon_qid: String,
    pub(crate) taxon_name: String,
    pub(crate) reference_qid: String,
    pub(crate) ref_title: Option<String>,
    pub(crate) ref_doi: Option<String>,
    pub(crate) pub_year: Option<i16>,
    pub(crate) statement: Option<String>,
}

impl From<CompoundEntry> for RowDto {
    fn from(value: CompoundEntry) -> Self {
        Self {
            compound_qid: value.compound_qid.to_string(),
            name: value.name.to_string(),
            inchikey: value.inchikey.map(|v| v.to_string()),
            smiles: value.smiles.map(|v| v.to_string()),
            mass: value.mass,
            formula: value.formula.map(|v| v.to_string()),
            taxon_qid: value.taxon_qid.to_string(),
            taxon_name: value.taxon_name.to_string(),
            reference_qid: value.reference_qid.to_string(),
            ref_title: value.ref_title.map(|v| v.to_string()),
            ref_doi: value.ref_doi.map(|v| v.to_string()),
            pub_year: value.pub_year,
            statement: value.statement.map(|v| v.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SearchResponse {
    pub(crate) resolved_taxon_qid: Option<String>,
    pub(crate) warning: Option<String>,
    pub(crate) query: String,
    pub(crate) rows: Vec<RowDto>,
    pub(crate) total_matches: usize,
    pub(crate) stats: SearchStats,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ExportUrlResponse {
    pub(crate) query: String,
    pub(crate) csv_url: String,
    pub(crate) json_url: String,
    pub(crate) rdf_url: String,
    pub(crate) csv_gz_url: String,
    pub(crate) json_gz_url: String,
    pub(crate) rdf_gz_url: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct ExportFileQuery {
    pub(crate) filename: Option<String>,
}
