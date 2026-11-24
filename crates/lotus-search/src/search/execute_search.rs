// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Capped `search`, `counts`, and top-level [`SearchError`].

use lotus_model::{DatasetStats, ValidationError};
use lotus_query::{counts_query, limit_query, parse_compounds_csv_capped, parse_counts_csv};

use super::query_plan::build_execution_query_with;
use super::resolve_structure::resolve_inputs;
use super::resolve_taxon::resolve_taxon;
use crate::client::Http;
use crate::error::{FetchError, ResponseFormat};
use crate::result::{SearchRequest, SearchResult};

/// How many rows a search returns when the caller does not say.
const DEFAULT_ROW_LIMIT: usize = 500;

/// Why a search could not be completed.
#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    /// The filters cannot be turned into a query.
    #[error(transparent)]
    Invalid(#[from] ValidationError),
    /// The endpoint failed. `stage` says which query was running.
    #[error("the {stage} query failed: {source}")]
    Transport {
        /// Which query was running: the taxon lookup or the results.
        stage: &'static str,
        /// What the endpoint said.
        #[source]
        source: FetchError,
    },
}

/// Run a search and return its rows, counts and the query that produced them.
///
/// # Errors
/// Returns [`FetchError`] if the endpoint cannot be reached, rejects the query,
/// or returns something that is not the CSV that was asked for. A failure to
/// read the counts is *not* an error: the rows are already in hand, and the
/// local count is the right answer to fall back on.
pub async fn search<H: Http>(
    http: &H,
    request: &SearchRequest,
) -> Result<SearchResult, SearchError> {
    let year_max = request.year_max;
    lotus_model::validate_criteria(&request.criteria, year_max).map_err(SearchError::Invalid)?;

    let taxon = resolve_taxon(http, &request.criteria.taxon)
        .await
        .map_err(|e| SearchError::Transport {
            stage: "taxon",
            source: e,
        })?;

    let resolved =
        resolve_inputs(http, &request.criteria)
            .await
            .map_err(|e| SearchError::Transport {
                stage: "structure or reference",
                source: e,
            })?;

    let query = build_execution_query_with(request, taxon.qid.as_deref(), &resolved);
    let limit = request.limit.unwrap_or(DEFAULT_ROW_LIMIT);
    // One row past the limit, so a result larger than the limit is visible as
    // one: an endpoint asked for exactly `limit` rows never sends more, and the
    // parser could not tell a full page from a cut one.
    let display_query = limit_query(&query, limit.saturating_add(1));

    let csv = crate::execute_with_fallback(http, &display_query, ResponseFormat::Csv)
        .await
        .map_err(|e| SearchError::Transport {
            stage: "results",
            source: e,
        })?
        .text()
        .map_err(|e| SearchError::Transport {
            stage: "results",
            source: e,
        })?;

    let (rows, stats, truncated) =
        parse_compounds_csv_capped(csv.as_bytes(), limit).map_err(|e| SearchError::Transport {
            stage: "results",
            source: e.into(),
        })?;

    // The count query is the expensive one, so it is the one that gets
    // rate-limited. When the rows are the whole result their counts are the
    // totals; when the limit cut them they describe one page, and reporting
    // them as totals would be wrong without saying so.
    let stats = match counts(http, &query).await {
        Ok(totals) => Some(totals),
        Err(_) if !truncated => Some(stats),
        Err(_) => None,
    };

    Ok(SearchResult {
        rows,
        stats,
        taxon: Some(taxon),
        query,
        truncated,
    })
}

/// The endpoint's own counts for a query.
///
/// # Errors
/// Returns [`FetchError`] if the count query could not be run or read. A caller
/// that already has the rows should treat this as advisory.
pub async fn counts<H: Http>(http: &H, query: &str) -> Result<DatasetStats, FetchError> {
    let csv = crate::execute_with_fallback(http, &counts_query(query), ResponseFormat::Csv)
        .await?
        .text()?;
    Ok(parse_counts_csv(csv.as_bytes())?)
}
