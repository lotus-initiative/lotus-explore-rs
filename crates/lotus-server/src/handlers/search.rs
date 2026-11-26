// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! `POST /v1/search` handler with cache lookup and single-flight coalescing.

use std::{sync::atomic::Ordering, time::Instant};

use axum::{Json, extract::State};
use tokio::time::timeout;

use super::prepare::{PreparedSearchRequest, acquire_upstream_permit, prepare_search_request};
use crate::{
    cache_key::build_search_cache_key,
    dto::{SearchRequest, SearchResponse},
    errors::{ApiError, ErrorResponse, SharedApiError},
    services::build_search_response,
    state::{AppState, search_cache_get, search_cache_put, search_inflight_guard},
};

fn attach_caller_context(
    mut response: SearchResponse,
    prepared: &PreparedSearchRequest,
) -> SearchResponse {
    response
        .resolved_taxon_qid
        .clone_from(&prepared.resolved_taxon_qid);
    response.warning.clone_from(&prepared.warning);
    response
}

async fn cached_search_response(
    state: &AppState,
    prepared: &PreparedSearchRequest,
) -> Result<SearchResponse, ApiError> {
    let cache_key = build_search_cache_key(
        &prepared.execution_query,
        prepared.limit,
        prepared.include_counts,
    );
    if let Some(cached) = search_cache_get(state, &cache_key) {
        state
            .metrics
            .search_cache_hits
            .fetch_add(1, Ordering::Relaxed);
        log::debug!("event=search state=cache_hit");
        return Ok(attach_caller_context(cached, prepared));
    }
    state
        .metrics
        .search_cache_misses
        .fetch_add(1, Ordering::Relaxed);
    log::debug!("event=search state=cache_miss");

    let guard = search_inflight_guard(state, &cache_key)?;
    if guard.is_leader() {
        state
            .metrics
            .search_upstream_hits
            .fetch_add(1, Ordering::Relaxed);
        log::debug!("event=search state=upstream_hit");
    } else {
        state
            .metrics
            .search_inflight_waits
            .fetch_add(1, Ordering::Relaxed);
        log::debug!("event=search state=coalesced_wait");
    }

    let metrics = state.metrics.clone();
    let request_timeout = state.request_timeout;
    let upstream_queue_wait = state.upstream_queue_wait;
    let upstream_permits = state.upstream_permits.clone();
    let execution_query = prepared.execution_query.clone();
    let limit = prepared.limit;
    let include_counts = prepared.include_counts;

    let response = guard
        .cell()
        .get_or_init(|| async move {
            let _upstream_permit =
                match acquire_upstream_permit(&upstream_permits, upstream_queue_wait, &metrics)
                    .await
                {
                    Ok(permit) => permit,
                    Err(err) => return Err(err),
                };
            timeout(
                request_timeout,
                build_search_response(&execution_query, limit, include_counts, None, None),
            )
            .await
            .map_err(|_| {
                metrics.request_timeouts.fetch_add(1, Ordering::Relaxed);
                log::warn!("event=search state=timeout phase=execution");
                SharedApiError::timed_out("search execution timed out")
            })?
            .map_err(SharedApiError::from)
        })
        .await
        .clone();
    guard.finish();

    match response {
        Ok(response) => {
            search_cache_put(state, cache_key, response.clone());
            Ok(attach_caller_context(response, prepared))
        }
        Err(err) => Err(err.into()),
    }
}

/// Execute a bounded, counted LOTUS search (`POST /v1/search`).
///
/// # Errors
/// Returns [`ApiError`] on invalid criteria, concurrency shedding, timeout, or upstream error.
#[utoipa::path(
    post,
    path = "/v1/search",
    request_body = SearchRequest,
    responses(
        (status = 200, description = "Search results", body = SearchResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 502, description = "Upstream SPARQL failure", body = ErrorResponse),
        (status = 503, description = "Server overloaded", body = ErrorResponse),
        (status = 504, description = "Search timeout", body = ErrorResponse)
    )
)]
pub async fn search(
    State(state): State<AppState>,
    Json(req): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, ApiError> {
    let req_started = Instant::now();
    let _permit = state.request_permits.try_acquire().map_err(|_| {
        state
            .metrics
            .overload_rejections
            .fetch_add(1, Ordering::Relaxed);
        log::warn!("event=search state=rejected reason=overloaded");
        ApiError::overloaded("Server is busy, retry shortly")
    })?;

    let prepared = prepare_search_request(&state, &req).await?;
    log::info!(
        "event=search state=start include_counts={} limit={} has_smiles={}",
        prepared.include_counts,
        prepared.limit,
        req.smiles
            .as_deref()
            .is_some_and(|smiles| !smiles.trim().is_empty()),
    );

    match cached_search_response(&state, &prepared).await {
        Ok(response) => {
            log::info!(
                "event=search state=success elapsed_ms={:.1} rows={} total_matches={}",
                req_started.elapsed().as_secs_f64() * 1000.0,
                response.rows.len(),
                response.total_matches,
            );
            Ok(Json(response))
        }
        Err(err) => {
            log::warn!(
                "event=search state=error elapsed_ms={:.1} status={} message={}",
                req_started.elapsed().as_secs_f64() * 1000.0,
                err.status,
                err.message,
            );
            Err(err)
        }
    }
}
