// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Upstream SPARQL execution and capped/counted response assembly.

use lotus_model::DatasetStats;
use lotus_search::{Endpoint, FetchError, ResponseFormat, Service, execute};

use crate::{
    errors::ApiError,
    types::{SearchResponse, SearchStats},
};
use lotus_explore_core::api::dto::RowDto;

async fn execute_qlever_csv_bytes(sparql: &str) -> Result<Vec<u8>, FetchError> {
    let http = lotus_search::reqwest_client::ReqwestClient::new()?;
    let answer = execute(
        &http,
        Endpoint::new(Service::Qlever),
        sparql,
        ResponseFormat::Csv,
    )
    .await?;
    Ok(answer.body)
}

/// Choose between the count query's totals and the exact totals computed from
/// the displayed rows.
pub fn resolve_count_stats(
    capped: bool,
    fallback_stats: DatasetStats,
    stats_result: Result<DatasetStats, ApiError>,
) -> Result<DatasetStats, ApiError> {
    match stats_result {
        Ok(stats) => Ok(stats),
        Err(err) if !capped => {
            log::warn!(
                "event=search state=count_fallback reason={} include_counts=true capped=false",
                err.message
            );
            Ok(fallback_stats)
        }
        Err(err) => Err(err),
    }
}

pub async fn build_search_response(
    execution_query: &str,
    limit: usize,
    include_counts: bool,
    resolved_taxon_qid: Option<String>,
    warning: Option<String>,
) -> Result<SearchResponse, ApiError> {
    let display_query = lotus_query::limit_query(execution_query, limit.saturating_add(1));
    let rows_future = async {
        let rows_bytes = execute_qlever_csv_bytes(&display_query)
            .await
            .map_err(|e| ApiError::from_fetch("display query failed", e))?;
        lotus_query::parse_compounds_csv_capped(&rows_bytes, limit)
            .map_err(|e| ApiError::upstream(format!("display parse failed: {e}")))
    };

    let (rows, stats) = if include_counts {
        let count_query = lotus_query::counts_query(execution_query);

        let stats_future = async {
            let count_bytes = execute_qlever_csv_bytes(&count_query)
                .await
                .map_err(|e| ApiError::from_fetch("count query failed", e))?;
            lotus_query::parse_counts_csv(&count_bytes)
                .map_err(|e| ApiError::upstream(format!("count parse failed: {e}")))
        };

        let (rows_result, stats_result) = tokio::join!(rows_future, stats_future);
        let (rows, fallback_stats, capped) = rows_result?;
        let stats = resolve_count_stats(capped, fallback_stats, stats_result)?;
        (rows, stats)
    } else {
        let (rows, stats, _) = rows_future.await?;
        (rows, stats)
    };

    Ok(SearchResponse {
        resolved_taxon_qid,
        warning,
        query: execution_query.to_string(),
        total_matches: stats.n_entries,
        stats: SearchStats::from(stats),
        rows: rows.into_iter().map(RowDto::from).collect(),
    })
}
