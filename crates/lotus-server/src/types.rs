// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! HTTP API request/response wire schemas, reusing shared enums and row DTOs
//! from [`lotus_explore_core::api::dto`].

use lotus_explore_core::api::dto::{ApiElementState, ApiSmilesSearchType, RowDto};
use lotus_model::DatasetStats;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// JSON response payload returned by `GET /health`.
#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: &'static str,
    pub uptime_secs: u64,
    pub search_cache_hits: u64,
    pub search_cache_misses: u64,
    pub search_inflight_waits: u64,
    pub search_upstream_hits: u64,
    pub export_cache_hits: u64,
    pub export_cache_misses: u64,
    pub export_inflight_waits: u64,
    pub export_upstream_hits: u64,
    pub overload_rejections: u64,
    /// Searches refused because every `QLever` slot was busy.
    pub upstream_shed: u64,
    pub request_timeouts: u64,
}

/// Request payload accepted by `POST /v1/search` and `POST /v1/export-url`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SearchRequest {
    pub taxon: Option<String>,
    /// A Wikidata QID (`Q23118`) or DOI (`10.7554/eLife.70780`, bare or `https://doi.org/…`)
    /// naming the reference to constrain by.
    pub reference: Option<String>,
    pub taxon_accepted_synonyms: Option<bool>,
    pub taxon_basionyms: Option<bool>,
    pub taxon_protonyms: Option<bool>,
    pub taxon_replacements: Option<bool>,
    #[serde(alias = "structure")]
    pub smiles: Option<String>,
    #[serde(alias = "structure_search")]
    pub smiles_search_type: Option<ApiSmilesSearchType>,
    /// Tanimoto cutoff for a similarity structure search.
    #[serde(alias = "smiles_threshold", alias = "structure_threshold")]
    pub similarity_threshold: Option<f64>,
    pub mass_min: Option<f64>,
    pub mass_max: Option<f64>,
    pub year_min: Option<u16>,
    pub year_max: Option<u16>,
    pub formula_exact: Option<String>,
    pub c_min: Option<u16>,
    pub c_max: Option<u16>,
    pub h_min: Option<u16>,
    pub h_max: Option<u16>,
    pub n_min: Option<u16>,
    pub n_max: Option<u16>,
    pub o_min: Option<u16>,
    pub o_max: Option<u16>,
    pub p_min: Option<u16>,
    pub p_max: Option<u16>,
    pub s_min: Option<u16>,
    pub s_max: Option<u16>,
    pub f_state: Option<ApiElementState>,
    pub cl_state: Option<ApiElementState>,
    pub br_state: Option<ApiElementState>,
    pub i_state: Option<ApiElementState>,
    pub limit: Option<usize>,
    pub include_counts: Option<bool>,
}

/// Dataset cardinality counters returned in [`SearchResponse`].
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SearchStats {
    #[serde(rename = "n_compounds")]
    pub compounds: usize,
    #[serde(rename = "n_taxa")]
    pub taxa: usize,
    #[serde(rename = "n_references")]
    pub references: usize,
    #[serde(rename = "n_entries")]
    pub entries: usize,
    #[serde(rename = "n_entries_unique")]
    pub unique_entries: usize,
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

/// JSON response payload returned by `POST /v1/search`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SearchResponse {
    pub resolved_taxon_qid: Option<String>,
    pub warning: Option<String>,
    pub query: String,
    pub rows: Vec<RowDto>,
    pub total_matches: usize,
    pub stats: SearchStats,
}

/// JSON response payload returned by `POST /v1/export-url`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ExportUrlResponse {
    pub query: String,
    pub csv_url: String,
    pub json_url: String,
    pub rdf_url: String,
    pub csv_gz_url: String,
    pub json_gz_url: String,
    pub rdf_gz_url: String,
}

/// Optional query parameters on `GET /v1/export-file/{cache_key}/{format}`.
#[derive(Debug, Default, Deserialize)]
pub struct ExportFileQuery {
    pub filename: Option<String>,
}
