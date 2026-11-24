// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! `POST /v1/export-url` and `GET /v1/export-file/{cache_key}/{format}` handlers.

use std::{
    sync::{Arc, atomic::Ordering},
    time::Instant,
};

use axum::{
    Json,
    body::Bytes,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::Response,
};
use flate2::{Compression, write::GzEncoder};
use lotus_query::{CsvColumnarReader, ExportFormat, RowExporter};
use lotus_search::Http as _;
use tokio::time::timeout;

use super::prepare::{PreparedExportRequest, acquire_upstream_permit, prepare_export_request};
use crate::{
    errors::{ApiError, ErrorResponse, SharedApiError},
    state::{
        AppState, export_cache_get, export_cache_put, export_inflight_cell, export_inflight_remove,
    },
    types::{ExportFileQuery, ExportUrlResponse, SearchRequest},
};

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
                csv_url: crate::export_urls::qlever_export_url(&query, ExportFormat::Csv),
                json_url: crate::export_urls::qlever_export_url(&query, ExportFormat::Json),
                rdf_url: crate::export_urls::qlever_export_url(&query, ExportFormat::Rdf),
                csv_gz_url: crate::export_urls::api_export_file_url(&cache_key, ExportFormat::Csv),
                json_gz_url: crate::export_urls::api_export_file_url(
                    &cache_key,
                    ExportFormat::Json,
                ),
                rdf_gz_url: crate::export_urls::api_export_file_url(&cache_key, ExportFormat::Rdf),
                query,
            })
        })
        .await
        .clone();
    export_inflight_remove(state, &prepared.cache_key, &cell);

    match response {
        Ok(response) => {
            export_cache_put(state, prepared.cache_key.clone(), response.clone());
            Ok(response)
        }
        Err(err) => Err(err.into()),
    }
}

/// Generate direct and proxied export download URLs (`POST /v1/export-url`).
///
/// # Errors
/// Returns [`ApiError`] on invalid criteria, concurrency shedding, timeout, or upstream error.
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

/// Stream a cached query export in CSV, JSON, or RDF/Turtle (`GET /v1/export-file/{cache_key}/{format}`).
///
/// # Errors
/// Returns [`ApiError`] on unknown key, invalid format, concurrency shedding, or upstream error.
#[utoipa::path(
    get,
    path = "/v1/export-file/{cache_key}/{format}",
    params(
        ("cache_key" = String, Path, description = "Export cache key returned by /v1/export-url"),
        ("format" = String, Path, description = "Export format: csv|json|ttl (rdf accepted for older links)"),
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
    let permit = Arc::clone(&state.request_permits)
        .try_acquire_owned()
        .map_err(|_| {
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

    let upstream_permit = acquire_upstream_permit(
        &state.upstream_permits,
        state.upstream_queue_wait,
        &state.metrics,
    )
    .await
    .map_err(ApiError::from)?;
    let select_url = crate::export_urls::qlever_export_url(&cached.query, ExportFormat::Csv);
    let http = lotus_search::reqwest_client::ReqwestClient::new()
        .map_err(|e| ApiError::upstream(format!("could not open the export client: {e}")))?;
    let response = timeout(
        state.request_timeout,
        http.get(&select_url, lotus_search::ResponseFormat::Csv.accept()),
    )
    .await
    .map_err(|_| {
        state
            .metrics
            .request_timeouts
            .fetch_add(1, Ordering::Relaxed);
        log::warn!("event=export_file state=timeout");
        ApiError::timed_out("export fetch timed out")
    })?
    .map_err(|e| ApiError::from_fetch("export fetch failed", e))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(ApiError::upstream(format!(
            "export upstream returned HTTP {status}"
        )));
    }

    let requested_filename = params
        .filename
        .as_deref()
        .map(lotus_query::sanitize_download_filename)
        .filter(|name| !name.is_empty());
    let compress = requested_filename.is_none();
    let content_type = if compress {
        "application/gzip".to_string()
    } else {
        format.content_type().to_string()
    };
    let attachment_name =
        requested_filename.unwrap_or_else(|| format!("{cache_key}.{}.gz", format.extension()));

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(8);
    tokio::spawn(async move {
        let _held = (permit, upstream_permit);
        pump_export(response, tx, format, compress).await;
    });

    let stream = futures::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
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
async fn pump_export<H>(
    mut response: H,
    tx: tokio::sync::mpsc::Sender<Result<Bytes, std::io::Error>>,
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

    if let (false, Some(reader)) = (failed, reader) {
        match reader.finish() {
            Ok(set) => {
                let mut exporter = RowExporter::new(format, &set);
                while let Some(chunk) = exporter.next_chunk() {
                    if !push_chunk(&tx, &mut encoder, chunk.as_bytes()).await {
                        break;
                    }
                }
            }
            Err(e) => {
                log("parse_failed", e.to_string());
                failed = true;
            }
        }
    }

    if failed {
        let _ = tx
            .send(Err(std::io::Error::other(
                "the export stopped before the endpoint finished sending it",
            )))
            .await;
        return;
    }

    if let Some(encoder) = encoder {
        let finished = encoder.finish().unwrap_or_default();
        if !finished.is_empty() {
            let _ = tx.send(Ok(Bytes::from(finished))).await;
        }
    }
}

async fn push_chunk(
    tx: &tokio::sync::mpsc::Sender<Result<Bytes, std::io::Error>>,
    encoder: &mut Option<GzEncoder<Vec<u8>>>,
    data: &[u8],
) -> bool {
    match encoder {
        Some(encoder) => {
            use std::io::Write as _;
            if encoder.write_all(data).is_err() || encoder.flush().is_err() {
                return false;
            }
            let ready = std::mem::take(encoder.get_mut());
            ready.is_empty() || tx.send(Ok(Bytes::from(ready))).await.is_ok()
        }
        None => tx.send(Ok(Bytes::from(data.to_vec()))).await.is_ok(),
    }
}

#[cfg(test)]
#[path = "export_query_shape.rs"]
mod export_query_shape;

#[cfg(test)]
#[path = "pump_tests.rs"]
mod pump_tests;
