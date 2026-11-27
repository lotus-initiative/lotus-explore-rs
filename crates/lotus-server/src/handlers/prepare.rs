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
    dto::SearchRequest,
    errors::{ApiError, SharedApiError},
    query_logic::{
        apply_request, build_execution_query_with, resolve_search_inputs, resolve_taxon_qid_cached,
    },
    state::{AppState, RuntimeMetrics, taxon_cache_get},
};

/// Retry delay advertised when upstream query slots are saturated.
pub(super) const UPSTREAM_RETRY_AFTER_SECS: u64 = 5;

/// Record overload metrics, log, and return the `503` response.
pub(super) fn reject_overloaded(state: &AppState, event: &'static str) -> ApiError {
    state
        .metrics
        .overload_rejections
        .fetch_add(1, Ordering::Relaxed);
    log::warn!("event={event} state=rejected reason=overloaded");
    ApiError::overloaded("Server is busy, retry shortly")
}

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

    let taxon_trimmed = criteria.taxon.trim();
    let taxon_needs_permit = !taxon_trimmed.is_empty()
        && taxon_trimmed != "*"
        && !lotus_model::is_qid(taxon_trimmed)
        && taxon_cache_get(state, &taxon_trimmed.to_lowercase()).is_none();
    let taxon_permit = if taxon_needs_permit {
        Some(
            acquire_upstream_permit(
                &state.upstream_permits,
                state.upstream_queue_wait,
                &state.metrics,
            )
            .await
            .map_err(ApiError::from)?,
        )
    } else {
        None
    };
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
    drop(taxon_permit);

    apply_resolved_taxon(&mut criteria, resolved_taxon_qid.as_deref());

    let structure = criteria.structure.trim();
    let reference = criteria.reference.trim();
    let structure_needs_lookup = !structure.is_empty()
        && criteria.structure_search == lotus_model::SmilesSearchType::Exact
        && !lotus_model::looks_like_a_compound_qid(structure);
    let reference_needs_lookup =
        !reference.is_empty() && !lotus_model::looks_like_reference_qid(reference);
    let inputs_need_permit = structure_needs_lookup || reference_needs_lookup;
    let inputs_permit = if inputs_need_permit {
        Some(
            acquire_upstream_permit(
                &state.upstream_permits,
                state.upstream_queue_wait,
                &state.metrics,
            )
            .await
            .map_err(ApiError::from)?,
        )
    } else {
        None
    };
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
    drop(inputs_permit);

    let execution_query =
        build_execution_query_with(&criteria, resolved_taxon_qid.as_deref(), &resolved_inputs);

    Ok(PreparedQuery {
        execution_query,
        resolved_taxon_qid,
        warning,
    })
}

pub fn apply_resolved_taxon(
    criteria: &mut lotus_model::SearchCriteria,
    resolved_taxon_qid: Option<&str>,
) {
    if let Some(qid) = resolved_taxon_qid
        && qid != "*"
    {
        criteria.taxon = qid.to_string();
    }
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
