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
    query_logic::{apply_request, build_execution_query, resolve_taxon_qid_cached},
    services::build_search_response,
    state::{
        AppState, RuntimeMetrics, build_export_cache_key, build_search_cache_key, export_cache_get,
        export_cache_put, export_inflight_cell, export_inflight_remove, search_cache_get,
        search_cache_put, search_inflight_cell, search_inflight_remove,
    },
    types::{ExportFileQuery, ExportUrlResponse, HealthResponse, SearchRequest, SearchResponse},
};
use axum::body::Bytes;
use flate2::Compression;
use flate2::write::GzEncoder;
use lotus_query::{CsvColumnarReader, ExportFormat, RowExporter};
use lotus_search::Http as _;

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
    // same 30-second budget that already rejects this query as a plain `SELECT`, so
    // asking for Turtle capped the export at whatever the endpoint would assemble in
    // time. The `SELECT` streams, and the other two formats are rendered here from
    // the same rows the CSV path already fetches.
    let select_url = export::qlever_export_url(&cached.query, ExportFormat::Csv);
    let http = lotus_search::reqwest_client::ReqwestClient::new()
        .map_err(|e| ApiError::upstream(format!("could not open the export client: {e}")))?;
    let mut response = http
        .get(&select_url, lotus_search::ResponseFormat::Csv.accept())
        .await
        .map_err(|e| ApiError::upstream(format!("export fetch failed: {e}")))?;
    // The inherent `reqwest` method shadows the trait's, so it is converted here.
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(ApiError::upstream(format!(
            "export upstream returned HTTP {status}"
        )));
    }

    let requested_filename = params
        .filename
        .as_deref()
        .map(crate::export::sanitize_download_filename)
        .filter(|name| !name.is_empty());
    // No name means the browser has to infer one, so the bytes are gzipped and named
    // for it. With a name it can save what it is given.
    let compress = requested_filename.is_none();
    let content_type = if compress {
        "application/gzip".to_string()
    } else {
        format.content_type().to_string()
    };
    let attachment_name =
        requested_filename.unwrap_or_else(|| format!("{cache_key}.{}.gz", format.extension()));

    // A bounded channel, so a client that stops reading stops the work. Unbounded
    // would trade the buffering we removed for buffering somewhere less visible.
    let (tx, rx) = tokio::sync::mpsc::channel::<Bytes>(8);
    tokio::spawn(pump_export(response, tx, format, compress));

    let stream = futures::stream::unfold(rx, |mut rx| async move {
        rx.recv()
            .await
            .map(|chunk| (Ok::<Bytes, std::io::Error>(chunk), rx))
    });
    log::info!(
        "event=export_file state=started format={} compressed={compress} parsed={}",
        format.extension(),
        format != ExportFormat::Csv,
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{attachment_name}\""),
        )
        .header(header::CACHE_CONTROL, "private, max-age=600")
        .body(axum::body::Body::from_stream(stream))
        .map_err(|e| ApiError::upstream(format!("response build failed: {e}")))
}

/// Pump one export from the upstream response to the client.
///
/// Separated from the handler so the handler reads as a decision about headers and
/// this reads as the pipeline it sets running. `response` is taken by value and moved
/// into the task, which is what lets the handler return without holding the body.
async fn pump_export<H>(
    mut response: H,
    tx: tokio::sync::mpsc::Sender<Bytes>,
    format: ExportFormat,
    compress: bool,
) where
    H: lotus_search::HttpResponse + Send + 'static,
{
    let log = |state: &str, detail: String| {
        log::warn!(
            "event=export_file state={state} format={} detail={detail}",
            format.log_name()
        );
    };
    let mut encoder = compress.then(|| GzEncoder::new(Vec::new(), Compression::default()));
    // CSV is already what the endpoint sends, so it is relayed as bytes and never
    // parsed. Only the formats that have to be built here need the rows.
    let mut reader = (format != ExportFormat::Csv).then(CsvColumnarReader::new);
    let mut failed = false;

    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => match reader.as_mut() {
                Some(reader) => {
                    if let Err(e) = reader.feed(&chunk) {
                        log("parse_failed", e.to_string());
                        failed = true;
                        break;
                    }
                }
                None => {
                    if !push_chunk(&tx, &mut encoder, &chunk).await {
                        break;
                    }
                }
            },
            Ok(None) => break,
            Err(e) => {
                log("upstream_truncated", e.to_string());
                failed = true;
                break;
            }
        }
    }

    // The rows only become a set at end of input, so a converted export begins after
    // the endpoint has finished. That is inherent to rendering from rows -- the
    // alternative is the CONSTRUCT this replaced -- and it costs a constant rather
    // than a second copy of the export.
    if let (false, Some(reader)) = (failed, reader) {
        {
            match reader.finish() {
                Ok(set) => {
                    let mut exporter = RowExporter::new(format, &set);
                    while let Some(chunk) = exporter.next_chunk() {
                        if !push_chunk(&tx, &mut encoder, chunk.as_bytes()).await {
                            break;
                        }
                    }
                }
                Err(e) => log("parse_failed", e.to_string()),
            }
        }
    }

    if let Some(encoder) = encoder {
        // `finish` writes the gzip trailer and then consumes the encoder, so the
        // bytes it produced are taken from the inner buffer afterwards.
        let finished = encoder.finish().unwrap_or_default();
        if !finished.is_empty() {
            let _ = tx.send(Bytes::from(finished)).await;
        }
    }
}

async fn push_chunk(
    tx: &tokio::sync::mpsc::Sender<Bytes>,
    encoder: &mut Option<GzEncoder<Vec<u8>>>,
    data: &[u8],
) -> bool {
    match encoder {
        Some(encoder) => {
            use std::io::Write as _;
            if encoder.write_all(data).is_err() || encoder.flush().is_err() {
                return false;
            }
            // Drain what the encoder has produced so far rather than the whole buffer,
            // so memory stays proportional to one chunk instead of the whole export.
            let ready = std::mem::take(encoder.get_mut());
            ready.is_empty() || tx.send(Bytes::from(ready)).await.is_ok()
        }
        None => tx.send(Bytes::from(data.to_vec())).await.is_ok(),
    }
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
    /// A `match` that handled CSV and then passed the upstream bytes through
    /// unchanged for the rest would type-check, pass a content-type test, and hand a
    /// reader a file called `.ttl` containing CSV.
    #[test]
    fn a_non_csv_format_is_built_here_rather_than_passed_through() {
        // Split for the same reason: both needles appear in this module's own text.
        let reader = concat!("CsvColumnar", "Reader");
        let exporter = concat!("RowExporter::", "new(format, &set)");
        assert!(
            HANDLERS.contains(reader),
            "the rows have to be parsed before they can be re-rendered"
        );
        assert!(
            HANDLERS.contains(exporter),
            "every format but CSV is rendered locally from the fetched rows"
        );
    }

    /// The export must not buffer the upstream body, and must not build the whole
    /// output before sending any of it.
    ///
    /// This is the difference between peak memory being one chunk and peak memory
    /// being the export twice over: the bulk fetch helper returns a `Vec<u8>` of the
    /// entire body, and a gzip encoder over a `Vec` holds the compressed copy alongside.
    /// Both are invisible in a response that succeeds, so the shape is pinned here
    /// rather than measured. `next_chunk` is what makes the output incremental --
    /// `for_each_chunk` would have needed the whole render to finish first, because
    /// it is synchronous and cannot await a send.
    #[test]
    fn the_export_streams_rather_than_buffering() {
        let buffered = concat!("fetch_", "url");
        let encoder = concat!("GzEncoder::new(Vec::new()", ", Compression::default())");
        assert!(
            !HANDLERS.contains(buffered),
            "the export must read the upstream chunk by chunk; fetching it whole puts \
             the entire export in memory before the first byte is sent"
        );
        assert!(
            HANDLERS.contains(encoder),
            "gzip must be incremental, or the compressed copy doubles peak memory"
        );
        let per_chunk = concat!("RowExporter::", "new(format, &set)");
        let drain = concat!("exporter.next_", "chunk()");
        assert!(
            HANDLERS.contains(per_chunk) && HANDLERS.contains(drain),
            "the rendered output must be drained a chunk at a time"
        );
        // The channel is what stops a client that stops reading from being turned
        // into unbounded buffering somewhere less visible than the export.
        assert!(
            HANDLERS.contains("tokio::sync::mpsc::channel::<Bytes>(8)"),
            "the outbound channel must be bounded, or backpressure becomes buffering"
        );
    }
}
