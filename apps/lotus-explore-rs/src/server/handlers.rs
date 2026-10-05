// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::Response,
};
use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore},
    time::timeout,
};

use crate::export;
use crate::server::{
    errors::{ApiError, ErrorResponse, SharedApiError},
    query_logic::{apply_request, build_execution_query, gzip_bytes, resolve_taxon_qid_cached},
    services::build_search_response,
    state::{
        AppState, RuntimeMetrics, build_export_cache_key, build_search_cache_key, export_cache_get,
        export_cache_put, export_inflight_cell, export_inflight_remove, search_cache_get,
        search_cache_put, search_inflight_cell, search_inflight_remove,
    },
    types::{ExportFileQuery, ExportUrlResponse, HealthResponse, SearchRequest, SearchResponse},
};
use lotus_query::{ExportFormat, parse_compounds_columnar};

#[utoipa::path(
    get,
    path = "/health",
    responses((status = 200, description = "Service health", body = HealthResponse))
)]
pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(state.metrics.snapshot())
}

#[utoipa::path(
    get,
    path = "/metrics",
    responses((status = 200, description = "Prometheus-style runtime metrics", body = String))
)]
pub async fn metrics(State(state): State<AppState>) -> Result<Response, ApiError> {
    // `Builder::body` only fails on an invalid header value or an unencodable
    // body; the headers below are static literals, so this is unreachable in
    // practice, but propagating a typed 500 `ApiError` is safer than panicking.
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .body(axum::body::Body::from(state.metrics.render_prometheus()))?)
}

struct PreparedSearchRequest {
    execution_query: String,
    resolved_taxon_qid: Option<String>,
    warning: Option<String>,
    limit: usize,
    include_counts: bool,
}

async fn prepare_search_request(
    state: &AppState,
    req: &SearchRequest,
) -> Result<PreparedSearchRequest, ApiError> {
    let mut criteria = apply_request(req)?;
    if !criteria.is_searchable() {
        return Err(ApiError::bad_request(
            "Either taxon or smiles/structure must be provided",
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
        log::warn!("event=search state=timeout phase=taxon");
        ApiError::upstream("taxon resolution timed out")
    })??;

    if let Some(qid) = resolved_taxon_qid.as_deref()
        && qid != "*"
    {
        criteria.taxon = qid.to_string();
    }

    Ok(PreparedSearchRequest {
        execution_query: build_execution_query(&criteria, resolved_taxon_qid.as_deref()),
        resolved_taxon_qid,
        warning,
        limit: req
            .limit
            .unwrap_or(state.default_limit)
            .clamp(1, crate::table_budget::API_MAX_ROWS),
        include_counts: req.include_counts.unwrap_or(true),
    })
}

/// Take one of the server's `QLever` query slots, or shed the search.
///
/// Deliberately not `max_concurrency`: that bounds what this server *accepts*,
/// this bounds what it asks of somebody else's public endpoint. `QLever` serves
/// Wikidata for everyone from one machine and cancels any query over 30 s, so a
/// handful in flight is a large ask and an unconstrained one holds its slot for
/// the whole budget.
///
/// Shedding with a `Retry-After` instead of queueing indefinitely: a caller told
/// to return in a few seconds stops lengthening the queue, and the endpoint never
/// sees the query at all.
async fn acquire_upstream_permit(
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

/// What a shed search tells the caller to wait for.
const UPSTREAM_RETRY_AFTER_SECS: u64 = 5;

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
        return Ok(cached);
    }
    state
        .metrics
        .search_cache_misses
        .fetch_add(1, Ordering::Relaxed);
    log::debug!("event=search state=cache_miss");

    let (cell, is_leader) = search_inflight_cell(state, &cache_key)?;
    if is_leader {
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
    let resolved_taxon_qid = prepared.resolved_taxon_qid.clone();
    let warning = prepared.warning.clone();
    let limit = prepared.limit;
    let include_counts = prepared.include_counts;

    let response = cell
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
                build_search_response(
                    &execution_query,
                    limit,
                    include_counts,
                    resolved_taxon_qid,
                    warning,
                ),
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
    search_inflight_remove(state, &cache_key, &cell, is_leader);

    match response {
        Ok(response) => {
            search_cache_put(state, cache_key, response.clone());
            Ok(response)
        }
        Err(err) => Err(err.into()),
    }
}

struct PreparedExportRequest {
    query: String,
    cache_key: String,
}

async fn prepare_export_request(
    state: &AppState,
    req: &SearchRequest,
) -> Result<PreparedExportRequest, ApiError> {
    let mut criteria = apply_request(req)?;
    if !criteria.is_searchable() {
        return Err(ApiError::bad_request(
            "Either taxon or smiles/structure must be provided",
        ));
    }

    let (resolved_taxon_qid, _warning) = timeout(
        state.request_timeout,
        resolve_taxon_qid_cached(state, criteria.taxon.clone()),
    )
    .await
    .map_err(|_| {
        state
            .metrics
            .request_timeouts
            .fetch_add(1, Ordering::Relaxed);
        log::warn!("event=export state=timeout phase=taxon");
        ApiError::upstream("taxon resolution timed out")
    })??;

    if let Some(qid) = resolved_taxon_qid.as_deref()
        && qid != "*"
    {
        criteria.taxon = qid.to_string();
    }

    let query = build_execution_query(&criteria, resolved_taxon_qid.as_deref());
    Ok(PreparedExportRequest {
        cache_key: build_export_cache_key(&query),
        query,
    })
}

async fn cached_export_urls(
    state: &AppState,
    prepared: &PreparedExportRequest,
) -> Result<ExportUrlResponse, ApiError> {
    if let Some(cached) = export_cache_get(state, &prepared.cache_key) {
        state
            .metrics
            .export_cache_hits
            .fetch_add(1, Ordering::Relaxed);
        log::debug!("event=export state=cache_hit");
        return Ok(cached);
    }
    state
        .metrics
        .export_cache_misses
        .fetch_add(1, Ordering::Relaxed);
    log::debug!("event=export state=cache_miss");

    let (cell, is_leader) = export_inflight_cell(state, &prepared.cache_key)?;
    if is_leader {
        state
            .metrics
            .export_upstream_hits
            .fetch_add(1, Ordering::Relaxed);
        log::debug!("event=export state=upstream_hit");
    } else {
        state
            .metrics
            .export_inflight_waits
            .fetch_add(1, Ordering::Relaxed);
        log::debug!("event=export state=coalesced_wait");
    }

    let query = prepared.query.clone();
    let cache_key = prepared.cache_key.clone();
    let response = cell
        .get_or_init(|| async move {
            Ok::<_, SharedApiError>(ExportUrlResponse {
                csv_url: export::qlever_export_url(&query, ExportFormat::Csv),
                json_url: export::qlever_export_url(&query, ExportFormat::Json),
                rdf_url: export::qlever_export_url(&query, ExportFormat::Rdf),
                csv_gz_url: export::api_export_file_url(&cache_key, ExportFormat::Csv),
                json_gz_url: export::api_export_file_url(&cache_key, ExportFormat::Json),
                rdf_gz_url: export::api_export_file_url(&cache_key, ExportFormat::Rdf),
                query,
            })
        })
        .await
        .clone();
    export_inflight_remove(state, &prepared.cache_key, &cell, is_leader);

    match response {
        Ok(response) => {
            export_cache_put(state, prepared.cache_key.clone(), response.clone());
            Ok(response)
        }
        Err(err) => Err(err.into()),
    }
}

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

#[utoipa::path(
    post,
    path = "/v1/export-url",
    request_body = SearchRequest,
    responses(
        (status = 200, description = "Direct export URLs", body = ExportUrlResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 502, description = "Upstream SPARQL failure", body = ErrorResponse),
        (status = 503, description = "Server overloaded", body = ErrorResponse),
        (status = 504, description = "Taxon-resolution timeout", body = ErrorResponse)
    )
)]
pub async fn export_urls(
    State(state): State<AppState>,
    Json(req): Json<SearchRequest>,
) -> Result<Json<ExportUrlResponse>, ApiError> {
    let req_started = Instant::now();
    let _permit = state.request_permits.try_acquire().map_err(|_| {
        state
            .metrics
            .overload_rejections
            .fetch_add(1, Ordering::Relaxed);
        log::warn!("event=export state=rejected reason=overloaded");
        ApiError::overloaded("Server is busy, retry shortly")
    })?;

    let prepared = prepare_export_request(&state, &req).await?;
    log::info!("event=export state=start");

    match cached_export_urls(&state, &prepared).await {
        Ok(response) => {
            log::info!(
                "event=export state=success elapsed_ms={:.1}",
                req_started.elapsed().as_secs_f64() * 1000.0,
            );
            Ok(Json(response))
        }
        Err(err) => {
            log::warn!(
                "event=export state=error elapsed_ms={:.1} status={} message={}",
                req_started.elapsed().as_secs_f64() * 1000.0,
                err.status,
                err.message,
            );
            Err(err)
        }
    }
}

#[utoipa::path(
    get,
    path = "/v1/export-file/{cache_key}/{format}",
    params(
        ("cache_key" = String, Path, description = "Export cache key returned by /v1/export-url"),
        ("format" = String, Path, description = "Export format: csv|json|rdf"),
        ("filename" = Option<String>, Query, description = "Optional direct filename (disables gzip wrapping)")
    ),
    responses(
        (status = 200, description = "Export file bytes"),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 502, description = "Upstream export failure", body = ErrorResponse),
        (status = 503, description = "Server overloaded", body = ErrorResponse),
        (status = 504, description = "Export fetch timeout", body = ErrorResponse)
    )
)]
pub async fn export_file(
    State(state): State<AppState>,
    Path((cache_key, format_raw)): Path<(String, String)>,
    Query(params): Query<ExportFileQuery>,
) -> Result<Response, ApiError> {
    let req_started = Instant::now();
    let _permit = state.request_permits.try_acquire().map_err(|_| {
        state
            .metrics
            .overload_rejections
            .fetch_add(1, Ordering::Relaxed);
        log::warn!("event=export_file state=rejected reason=overloaded");
        ApiError::overloaded("Server is busy, retry shortly")
    })?;

    let format = ExportFormat::parse(&format_raw)
        .ok_or_else(|| ApiError::bad_request("Unsupported export format"))?;
    let cached = export_cache_get(&state, &cache_key).ok_or_else(|| {
        ApiError::bad_request("Export link expired or is unknown. Regenerate the export URL.")
    })?;

    // Always ask QLever for the `SELECT`, never for a `CONSTRUCT`.
    //
    // A `CONSTRUCT` is materialised by the endpoint before a byte is sent, under the
    // same 30-second budget that already rejects this query as a plain `SELECT`. So
    // asking for Turtle capped the export at whatever the endpoint would assemble in
    // time, which is not a limit we control and is far below the row count the CSV
    // path reaches. The `SELECT` streams, and the Turtle is built here from the same
    // rows the CSV path already fetches -- `RowExporter` is the renderer the CLI and
    // the in-browser download both use, so all three produce the same graph.
    let select_url = export::qlever_export_url(&cached.query, ExportFormat::Csv);
    let http = lotus_search::reqwest_client::ReqwestClient::new()
        .map_err(|e| ApiError::upstream(format!("could not open the export client: {e}")))?;
    let select_bytes = timeout(
        state.request_timeout,
        lotus_search::fetch_url(&http, &select_url, lotus_search::ResponseFormat::Csv),
    )
    .await
    .map_err(|_| {
        state
            .metrics
            .request_timeouts
            .fetch_add(1, Ordering::Relaxed);
        log::warn!("event=export_file state=timeout phase=fetch format={format_raw}");
        ApiError::upstream("export fetch timed out")
    })?
    .map_err(|e| ApiError::upstream(format!("export fetch failed: {e}")))?;
    let raw_bytes = if format == ExportFormat::Csv {
        select_bytes
    } else {
        let set = parse_compounds_columnar(std::io::Cursor::new(select_bytes.as_slice()))
            .map_err(|e| ApiError::upstream(format!("export could not be read: {e}")))?;
        let mut out: Vec<u8> = Vec::new();
        lotus_query::RowExporter::new(format, &set).for_each_chunk(|chunk| {
            out.extend_from_slice(chunk.as_bytes());
        });
        out
    };
    let raw_len = raw_bytes.len();

    let requested_filename = params
        .filename
        .as_deref()
        .map(crate::export::sanitize_download_filename)
        .filter(|name| !name.is_empty());

    let (body_bytes, content_type, attachment_name) = if let Some(filename) = requested_filename {
        (raw_bytes, format.content_type(), filename)
    } else {
        let gz_bytes = gzip_bytes(&raw_bytes)
            .map_err(|e| ApiError::upstream(format!("gzip encoding failed: {e}")))?;
        (
            gz_bytes,
            "application/gzip",
            format!("{cache_key}.{}.gz", format.extension()),
        )
    };
    log::info!(
        "event=export_file state=success elapsed_ms={:.1} format={} raw_bytes={} out_bytes={} named={}",
        req_started.elapsed().as_secs_f64() * 1000.0,
        format.extension(),
        raw_len,
        body_bytes.len(),
        params.filename.is_some(),
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{attachment_name}\""),
        )
        .header(header::CACHE_CONTROL, "private, max-age=600")
        .body(axum::body::Body::from(body_bytes))
        .map_err(|e| ApiError::upstream(format!("response build failed: {e}")))
}

/// Gates on the export handler's use of the endpoint.
///
/// Source-level rather than behavioural, because the behaviour being pinned is
/// *which query shape is sent upstream*, and a mock endpoint would have to answer a
/// `CONSTRUCT` to prove it -- so the test would be asserting against the thing it
/// is trying to forbid. Reading the source is the only way to see the request that
/// would be made.
#[cfg(test)]
mod export_query_shape {
    const HANDLERS: &str = include_str!("handlers.rs");

    /// The export path must never ask `QLever` for a `CONSTRUCT`.
    ///
    /// This is the whole point of the change and it is invisible in the response: a
    /// `CONSTRUCT` works, returns correct Turtle, and is simply capped at whatever
    /// the endpoint will materialise inside 30 seconds. Nothing fails when the cap
    /// is hit except an export that is quietly smaller than the one asked for, which
    /// is the failure mode a reader is least likely to notice and most likely to
    /// spend hours on.
    #[test]
    fn the_export_never_asks_the_endpoint_for_a_construct() {
        // The method that wraps a SELECT in a CONSTRUCT for the Rdf format is reachable
        // only through the URL builder, and only bites when handed that format. Called
        // directly here with Rdf, the cap is back.
        //
        // The needles are assembled at compile time rather than written out: this
        // module is part of the file it searches, so a literal would match its own
        // assertion and the gate would pass forever.
        let prepared = concat!("prepared_", "query");
        assert!(
            !HANDLERS.contains(prepared),
            "the export path must render Turtle locally, not ask for the CONSTRUCT \
             that wrapping implies"
        );
        assert!(
            HANDLERS.contains("qlever_export_url(&cached.query, ExportFormat::Csv)"),
            "the export must fetch the SELECT; asking for any other format upstream \
             reintroduces endpoint-side materialisation"
        );
    }

    /// Every non-CSV format has to be produced here, from rows already fetched.
    ///
    /// A `match` that handled CSV and then returned the `SELECT` bytes unchanged for
    /// the rest would type-check, pass a content-type test, and hand a reader a file
    /// called `.ttl` containing CSV.
    #[test]
    fn a_non_csv_format_is_built_here_rather_than_passed_through() {
        // Split for the same reason: both needles appear in this module's own text.
        let exporter = concat!("RowExporter::", "new(format, &set)");
        let parser = concat!("parse_compounds_", "columnar");
        assert!(
            HANDLERS.contains(exporter),
            "every format but CSV is rendered locally from the fetched rows"
        );
        assert!(
            HANDLERS.contains(parser),
            "the rows have to be parsed before they can be re-rendered"
        );
    }
}
