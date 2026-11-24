// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! HTTP API request/response wire schemas, reusing shared enums and row DTOs
//! from [`lotus_explore_core::api::dto`].

pub use lotus_explore_core::api::dto::{ApiElementState, ApiSmilesSearchType, RowDto};
use lotus_model::DatasetStats;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// JSON response payload returned by `GET /health`.
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
    /// Searches refused because every `QLever` slot was busy.
    pub(crate) upstream_shed: u64,
    pub(crate) request_timeouts: u64,
}

/// Request payload accepted by `POST /v1/search` and `POST /v1/export-url`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SearchRequest {
    pub(crate) taxon: Option<String>,
    /// A Wikidata QID (`Q23118`) or DOI (`10.7554/eLife.70780`, bare or `https://doi.org/…`)
    /// naming the reference to constrain by.
    pub(crate) reference: Option<String>,
    pub(crate) taxon_accepted_synonyms: Option<bool>,
    pub(crate) taxon_basionyms: Option<bool>,
    pub(crate) taxon_protonyms: Option<bool>,
    pub(crate) taxon_replacements: Option<bool>,
    #[serde(alias = "structure")]
    pub(crate) smiles: Option<String>,
    #[serde(alias = "structure_search")]
    pub(crate) smiles_search_type: Option<ApiSmilesSearchType>,
    /// Tanimoto cutoff for a similarity structure search.
    #[serde(alias = "smiles_threshold", alias = "structure_threshold")]
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

/// Dataset cardinality counters returned in [`SearchResponse`].
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

/// JSON response payload returned by `POST /v1/search`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SearchResponse {
    pub(crate) resolved_taxon_qid: Option<String>,
    pub(crate) warning: Option<String>,
    pub(crate) query: String,
    pub(crate) rows: Vec<RowDto>,
    pub(crate) total_matches: usize,
    pub(crate) stats: SearchStats,
}

/// JSON response payload returned by `POST /v1/export-url`.
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

/// Optional query parameters on `GET /v1/export-file/{cache_key}/{format}`.
#[derive(Debug, Default, Deserialize)]
pub struct ExportFileQuery {
    pub(crate) filename: Option<String>,
}
