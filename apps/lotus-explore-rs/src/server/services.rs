// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::server::{
    errors::ApiError,
    types::{RowDto, SearchResponse, SearchStats},
};
use crate::{models::DatasetStats, sparql};

pub async fn build_search_response(
    execution_query: &str,
    limit: usize,
    include_counts: bool,
    resolved_taxon_qid: Option<String>,
    warning: Option<String>,
) -> Result<SearchResponse, ApiError> {
    let display_query = lotus_query::limit_query(execution_query, limit);

    let (rows, stats) = if include_counts {
        let count_query = lotus_query::counts_query(execution_query);

        let rows_future = async {
            let rows_bytes = sparql::execute_sparql_bytes(&display_query)
                .await
                .map_err(|e| ApiError::upstream(format!("display query failed: {e}")))?;
            lotus_query::parse_compounds_csv(&rows_bytes, limit)
                .map_err(|e| ApiError::upstream(format!("display parse failed: {e}")))
        };
        let stats_future = async {
            let count_bytes = sparql::execute_sparql_bytes(&count_query)
                .await
                .map_err(|e| ApiError::upstream(format!("count query failed: {e}")))?;
            lotus_query::parse_counts_csv(&count_bytes)
                .map_err(|e| ApiError::upstream(format!("count parse failed: {e}")))
        };

        let (rows_result, stats_result) = tokio::join!(rows_future, stats_future);
        let rows = rows_result?;
        let stats = match stats_result {
            Ok(stats) => stats,
            Err(err) => {
                log::warn!(
                    "event=search state=count_fallback reason={} include_counts=true",
                    err.message
                );
                DatasetStats::from_entries(&rows)
            }
        };
        (rows, stats)
    } else {
        let rows_bytes = sparql::execute_sparql_bytes(&display_query)
            .await
            .map_err(|e| ApiError::upstream(format!("display query failed: {e}")))?;
        let rows = lotus_query::parse_compounds_csv(&rows_bytes, limit)
            .map_err(|e| ApiError::upstream(format!("display parse failed: {e}")))?;
        let stats = DatasetStats::from_entries(&rows);
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
