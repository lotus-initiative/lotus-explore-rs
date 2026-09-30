// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::download::ExportTimerLabel;
use crate::perf;
use crate::sparql;
use crate::sparql::wdqs_download_query;
use lotus_query::ExportFormat as DownloadFormat;
use lotus_search::ResponseFormat as LotusResponseFormat;
use lotus_search::WDQS_SCHOLARLY;
use std::sync::Arc;

pub(super) async fn execute_download_with_fallback(
    format: DownloadFormat,
    query: Arc<str>,
    filename: String,
    dl_timer: perf::TimerHandle,
) -> Result<String, String> {
    // First try QLever, fallback to WDQS on 502
    let result = execute_download_direct(format, query.as_ref()).await;

    match result {
        Ok(body) => finalize_download(format, "direct", &body, dl_timer, &filename),
        Err(e) => {
            // Check if it's a gateway error (502) indicating QLever is down
            if e.contains("502") || e.contains("Bad Gateway") || e.contains("gateway") {
                log::warn!(
                    "event=download format={} phase=fetch state=fallback reason=qlever_502",
                    format.log_name()
                );
                // Retry with WDQS using scholarly subgraph query
                execute_download_wdqs(format, query, filename, dl_timer).await
            } else {
                handle_download_error(format, &e, dl_timer)
            }
        }
    }
}

/// Execute a download against a WDQS endpoint with the given query and endpoint.
async fn execute_download_wdqs_endpoint(
    format: DownloadFormat,
    query: &str,
    endpoint: &str,
    filename: &str,
    dl_timer: perf::TimerHandle,
) -> Result<String, String> {
    let prepared = format.prepared_query(query);
    execute_sparql_with_format_download(
        format,
        &prepared,
        endpoint,
        super::wdqs_response_format(format),
        filename,
        dl_timer,
    )
    .await
}

async fn execute_download_wdqs(
    format: DownloadFormat,
    query: Arc<str>,
    filename: String,
    dl_timer: perf::TimerHandle,
) -> Result<String, String> {
    let (endpoint, wdqs_query) = wdqs_download_query(&query);
    if endpoint == WDQS_SCHOLARLY {
        log::warn!(
            "event=download format={} phase=fetch state=wdqs_scholarly_endpoint",
            format.log_name()
        );
    }
    execute_download_wdqs_endpoint(format, &wdqs_query, endpoint, &filename, dl_timer).await
}

/// Logs fetch timing, triggers the browser download, and logs trigger timing.
/// Shared between the `QLever` and WDQS download paths.
fn finalize_download(
    format: DownloadFormat,
    source: &str,
    body: &str,
    dl_timer: perf::TimerHandle,
    filename: &str,
) -> Result<String, String> {
    let fetch_elapsed = perf::end_timer(format.timer_label(), dl_timer);
    perf::log_timing(
        "download",
        &format!(
            "event=download format={} phase=fetch state=success source={source} body_bytes={}",
            format.log_name(),
            body.len()
        ),
        Some(fetch_elapsed),
    );

    let trigger_timer = perf::start_timer(&format.trigger_timer_label());
    let written = trigger_download(filename, format.content_type(), body);
    let trigger_elapsed = perf::end_timer(&format.trigger_timer_label(), trigger_timer);
    match &written {
        Ok(_) => perf::log_timing(
            "download",
            &format!(
                "event=download format={} phase=trigger state=success source={source}",
                format.log_name()
            ),
            Some(trigger_elapsed),
        ),
        Err(e) => perf::log_timing(
            "download",
            &format!(
                "event=download format={} phase=trigger state=error source={source} reason={e}",
                format.log_name()
            ),
            Some(trigger_elapsed),
        ),
    }
    written
}

fn handle_download_error(
    format: DownloadFormat,
    error: &str,
    dl_timer: perf::TimerHandle,
) -> Result<String, String> {
    let elapsed = perf::end_timer(format.timer_label(), dl_timer);
    perf::log_timing(
        "download",
        &format!(
            "event=download format={} phase=fetch state=error source=direct reason={}",
            format.log_name(),
            error
        ),
        Some(elapsed),
    );
    log::warn!(
        "event=download format={} phase=fetch state=error source=direct reason={}",
        format.log_name(),
        error
    );
    Err(error.to_string())
}

async fn execute_download_direct(format: DownloadFormat, query: &str) -> Result<String, String> {
    let prepared_query = format.prepared_query(query);
    match format {
        DownloadFormat::Csv => sparql::execute_query(&prepared_query)
            .await
            .map_err(|e| e.to_string()),
        DownloadFormat::Json => {
            sparql::execute_sparql_format(&prepared_query, LotusResponseFormat::SparqlJson)
                .await
                .map_err(|e| e.to_string())
        }
        DownloadFormat::Rdf => {
            sparql::execute_sparql_format(&prepared_query, LotusResponseFormat::Turtle)
                .await
                .map_err(|e| e.to_string())
        }
    }
}

async fn execute_sparql_with_format_download(
    format: DownloadFormat,
    query: &str,
    endpoint: &str,
    response_format: LotusResponseFormat,
    filename: &str,
    dl_timer: perf::TimerHandle,
) -> Result<String, String> {
    use crate::sparql::execute_sparql_format_at as shared_execute;

    let body = shared_execute(query, endpoint, response_format)
        .await
        .map_err(|e| e.to_string())?;

    finalize_download(format, "wdqs", &body, dl_timer, filename)
}

/// Returns a message naming the file that was written, for display to the user.
pub(super) fn trigger_download(
    filename: &str,
    mime: &str,
    content: &str,
) -> Result<String, String> {
    let _ = mime;
    // The result used to be dropped here, which turned a failed export into a
    // silent no-op: the user clicked download, nothing appeared, and no error
    // was ever logged.
    match super::local_file::download_text(content, filename) {
        Ok(path) => {
            let message = path.display().to_string();
            log::info!("event=download phase=trigger state=success path={message}");
            Ok(message)
        }
        Err(e) => {
            log::error!("event=download phase=trigger state=error source=native reason={e}");
            Err(e)
        }
    }
}
