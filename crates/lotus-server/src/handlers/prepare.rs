// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Shared request validation, upstream input resolution, and polite permit acquisition.

use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore},
    time::timeout,
};

use crate::{
    cache_key::build_export_cache_key,
    errors::{ApiError, SharedApiError},
    query_logic::{
        apply_request, build_execution_query_with, resolve_search_inputs, resolve_taxon_qid_cached,
    },
    state::{AppState, RuntimeMetrics},
    types::SearchRequest,
};

/// Retry delay advertised when upstream query slots are saturated.
const UPSTREAM_RETRY_AFTER_SECS: u64 = 5;

struct PreparedQuery {
    execution_query: String,
    resolved_taxon_qid: Option<String>,
    warning: Option<String>,
}

pub(super) struct PreparedSearchRequest {
    pub(super) execution_query: String,
    pub(super) resolved_taxon_qid: Option<String>,
    pub(super) warning: Option<String>,
    pub(super) limit: usize,
    pub(super) include_counts: bool,
}

pub(super) struct PreparedExportRequest {
    pub(super) query: String,
    pub(super) cache_key: String,
}

async fn prepare_query(
    state: &AppState,
    req: &SearchRequest,
    event: &'static str,
) -> Result<PreparedQuery, ApiError> {
    let mut criteria = apply_request(req)?;
    if !criteria.is_searchable() {
        return Err(ApiError::bad_request(
            "Either taxon, smiles/structure, or reference must be provided",
        ));
    }

    let (resolved_taxon_qid, warning) = timeout(
        state.request_timeout,
        resolve_taxon_qid_cached(state, criteria.taxon.clone()),
    )
    .await
    .map_err(|_| {
        state
            .metrics
            .request_timeouts
            .fetch_add(1, Ordering::Relaxed);
        log::warn!("event={event} state=timeout phase=taxon");
        ApiError::timed_out("taxon resolution timed out")
    })??;

    if let Some(qid) = resolved_taxon_qid.as_deref()
        && qid != "*"
    {
        criteria.taxon = qid.to_string();
    }

    let resolved_inputs = timeout(state.request_timeout, resolve_search_inputs(&criteria))
        .await
        .map_err(|_| {
            state
                .metrics
                .request_timeouts
                .fetch_add(1, Ordering::Relaxed);
            log::warn!("event={event} state=timeout phase=inputs");
            ApiError::timed_out("input resolution timed out")
        })??;

    let execution_query =
        build_execution_query_with(&criteria, resolved_taxon_qid.as_deref(), &resolved_inputs);

    Ok(PreparedQuery {
        execution_query,
        resolved_taxon_qid,
        warning,
    })
}

pub(super) async fn prepare_search_request(
    state: &AppState,
    req: &SearchRequest,
) -> Result<PreparedSearchRequest, ApiError> {
    let prepared = prepare_query(state, req, "search").await?;
    Ok(PreparedSearchRequest {
        execution_query: prepared.execution_query,
        resolved_taxon_qid: prepared.resolved_taxon_qid,
        warning: prepared.warning,
        limit: req
            .limit
            .unwrap_or(state.default_limit)
            .clamp(1, lotus_explore_core::table_budget::API_MAX_ROWS),
        include_counts: req.include_counts.unwrap_or(true),
    })
}

pub(super) async fn prepare_export_request(
    state: &AppState,
    req: &SearchRequest,
) -> Result<PreparedExportRequest, ApiError> {
    let prepared = prepare_query(state, req, "export").await?;
    Ok(PreparedExportRequest {
        cache_key: build_export_cache_key(&prepared.execution_query),
        query: prepared.execution_query,
    })
}

/// Acquire one upstream `QLever` query slot, or shed with `503 Retry-After`.
pub(super) async fn acquire_upstream_permit(
    permits: &Arc<Semaphore>,
    wait: Duration,
    metrics: &RuntimeMetrics,
) -> Result<OwnedSemaphorePermit, SharedApiError> {
    match timeout(wait, Arc::clone(permits).acquire_owned()).await {
        Ok(Ok(permit)) => Ok(permit),
        Ok(Err(_closed)) => Err(SharedApiError::from(ApiError::internal(
            "the server is shutting down",
        ))),
        Err(_) => {
            metrics.upstream_shed.fetch_add(1, Ordering::Relaxed);
            log::warn!(
                "event=search state=shed reason=upstream_budget wait_ms={}",
                wait.as_millis()
            );
            Err(SharedApiError::from(ApiError::upstream_overloaded(
                "Every query slot to the public endpoint is busy. Retry in a few seconds.",
                UPSTREAM_RETRY_AFTER_SECS,
            )))
        }
    }
}
