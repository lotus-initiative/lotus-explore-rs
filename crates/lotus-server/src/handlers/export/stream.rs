// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! `GET /v1/export-file/{cache_key}/{format}` streaming handler and chunk pump.

use std::sync::{Arc, atomic::Ordering};

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::Response,
};
use flate2::{Compression, write::GzEncoder};
use lotus_query::{CsvColumnarReader, ExportFormat, RowExporter};
use lotus_search::Http as _;
use tokio::time::timeout;

use super::super::prepare::{
    UPSTREAM_RETRY_AFTER_SECS, acquire_upstream_permit, reject_overloaded,
};
use crate::{
    dto::ExportFileQuery,
    errors::{ApiError, ErrorResponse},
    state::{AppState, export_cache_get},
};

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
        .map_err(|_| reject_overloaded(&state, "export_file"))?;

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
    let endpoint = lotus_search::Endpoint::new(lotus_search::Service::Qlever);
    let form_body = format!(
        "query={}&action={}",
        lotus_model::percent_encode(&cached.query),
        ExportFormat::Csv.qlever_action(),
    );
    let http = lotus_search::reqwest_client::ReqwestClient::new()
        .map_err(|e| ApiError::upstream(format!("could not open the export client: {e}")))?;
    let response = timeout(
        state.request_timeout,
        http.post(
            endpoint.url(),
            lotus_search::ResponseFormat::Csv.accept(),
            form_body,
        ),
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
    check_upstream_status(status)?;

    let requested_filename = sanitized_attachment(params.filename.as_deref());
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
    let mut aborted = false;

    loop {
        tokio::select! {
            biased;
            () = tx.closed() => {
                aborted = true;
                break;
            }
            chunk = response.chunk() => match chunk {
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
                            aborted = true;
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
    }

    if aborted {
        return;
    }

    if let (false, Some(reader)) = (failed, reader) {
        match reader.finish() {
            Ok(set) => {
                let mut exporter = RowExporter::new(format, &set);
                while let Some(chunk) = exporter.next_chunk() {
                    if !push_chunk(&tx, &mut encoder, chunk.as_bytes()).await {
                        aborted = true;
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

    if aborted {
        return;
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
        match encoder.finish() {
            Ok(finished) => {
                if !finished.is_empty() {
                    let _ = tx.send(Ok(Bytes::from(finished))).await;
                }
            }
            Err(err) => {
                log("gzip_finish_failed", err.to_string());
                let _ = tx.send(Err(err)).await;
            }
        }
    }
}

async fn push_chunk(
    tx: &tokio::sync::mpsc::Sender<Result<Bytes, std::io::Error>>,
    encoder: &mut Option<GzEncoder<Vec<u8>>>,
    data: &[u8],
) -> bool {
    if data.is_empty() {
        return true;
    }
    match encoder {
        Some(encoder) => {
            use std::io::Write as _;
            if encoder
                .write_all(data)
                .and_then(|()| encoder.flush())
                .is_err()
            {
                return false;
            }
            let ready = std::mem::take(encoder.get_mut());
            tx.send(Ok(Bytes::from(ready))).await.is_ok()
        }
        None => tx.send(Ok(Bytes::from(data.to_vec()))).await.is_ok(),
    }
}

fn check_upstream_status(status: u16) -> Result<(), ApiError> {
    if (200..300).contains(&status) {
        return Ok(());
    }
    if matches!(status, 408 | 504) {
        return Err(ApiError::timed_out(format!(
            "export upstream timed out with HTTP {status}"
        )));
    }
    if matches!(status, 429 | 503) {
        return Err(ApiError::upstream_overloaded(
            format!("export upstream rate-limited or unavailable (HTTP {status})"),
            UPSTREAM_RETRY_AFTER_SECS,
        ));
    }
    Err(ApiError::upstream(format!(
        "export upstream returned HTTP {status}"
    )))
}

fn sanitized_attachment(filename: Option<&str>) -> Option<String> {
    filename
        .map(lotus_query::sanitize_download_filename)
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
#[path = "stream/tests.rs"]
mod tests;
