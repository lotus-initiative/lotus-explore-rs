// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::api;
use crate::download::{export_timer_label, export_trigger_timer_label};
use crate::perf;
use crate::repositories::is_wdqs_fallback_used;
use crate::sparql::wdqs_download_query;
use lotus_query::ExportFormat as DownloadFormat;
use lotus_search::QLEVER_WIKIDATA;
use lotus_search::SearchCriteria;
use std::sync::Arc;

pub(super) async fn execute_download_wasm(
    format: DownloadFormat,
    criteria: Arc<SearchCriteria>,
    query: Arc<str>,
    filename: String,
    dl_timer: perf::TimerHandle,
) -> Result<(), String> {
    // If WDQS fallback was used for the interactive query, download from WDQS directly
    if is_wdqs_fallback_used() {
        log::warn!(
            "event=download format={} phase=fetch state=wdqs_fallback fallback=true",
            format.log_name()
        );
        return execute_download_wasm_wdqs(format, query, filename, dl_timer).await;
    }

    match api::export_urls(&criteria).await {
        Ok(urls) => {
            let url = append_filename_query(select_export_url(format, &urls), &filename);
            let fetch_elapsed = perf::end_timer(export_timer_label(format), dl_timer);
            perf::log_timing(
                "download",
                &format!(
                    "event=download format={} phase=fetch state=success source=api_url",
                    format.log_name()
                ),
                Some(fetch_elapsed),
            );

            let trigger_timer = perf::start_timer(&export_trigger_timer_label(format));
            let _ = crate::upload::download_url(&url, &filename);
            let trigger_elapsed =
                perf::end_timer(&export_trigger_timer_label(format), trigger_timer);
            perf::log_timing(
                "download",
                &format!(
                    "event=download format={} phase=trigger state=success source=api_url",
                    format.log_name()
                ),
                Some(trigger_elapsed),
            );
            Ok(())
        }
        Err(err) => {
            log::warn!(
                "event=download format={} phase=fetch state=fallback reason=api_export_urls_failed detail={err}",
                format.log_name()
            );
            if is_wdqs_fallback_used() {
                log::warn!(
                    "event=download format={} phase=fetch state=wdqs_fallback_from_api_error",
                    format.log_name()
                );
                execute_download_wasm_wdqs(format, query, filename, dl_timer).await
            } else {
                execute_download_wasm_browser_post(format, query, filename, dl_timer).await
            }
        }
    }
}

/// Stream an export from WDQS into a file, without holding it in memory.
///
/// This is the one download path that has to run *through* the module, and the
/// reason is a browser limitation rather than a design choice: WDQS answers a GET
/// with `text/csv` and no `Content-Disposition`, so a link or a form POST renders
/// the CSV in a tab instead of saving it, and a cross-origin `download` attribute
/// is ignored by every browser. Fetching it here and building a `Blob` is what
/// forces the save and the filename.
///
/// The obvious version of that decodes the whole body into one `String` first,
/// which for a two-million-row export means roughly 600 MB of CSV held three times
/// over -- raw bytes, decoded string, `Blob` -- in a tab budgeted for 200. So the
/// body is read through [`BodyChunks`](lotus_search::BodyChunks) and each chunk is
/// copied into JavaScript-owned memory and dropped from the WebAssembly heap
/// before the next arrives, so peak module memory is one chunk.
///
/// The other two paths do not have this problem and do not come here: an API
/// export URL and a `QLever` form POST are both handed to the browser, which
/// streams them itself and never puts a byte in this module's memory.
async fn execute_download_wasm_wdqs(
    format: DownloadFormat,
    query: Arc<str>,
    filename: String,
    dl_timer: perf::TimerHandle,
) -> Result<(), String> {
    // For simple reference queries, use scholarly endpoint directly without transformation
    let (endpoint, wdqs_query) = wdqs_download_query(&query);

    // For RDF format, the query must be wrapped in CONSTRUCT
    // (WDQS can't return Turtle for SELECT queries)
    let prepared_query = format.prepared_query(&wdqs_query);

    // Determine the WDQS response format
    let response_format = super::wdqs_response_format(format);

    let mut body =
        crate::sparql::execute_sparql_chunks_at(&prepared_query, endpoint, response_format)
            .await
            .map_err(|e| e.to_string())?;

    // Copied into the JS heap per chunk, with the Rust buffer dropped before the
    // next is read. `with_capacity` is a guess: a chunk per read is typical, and
    // a wrong guess costs a reallocation of pointers rather than of bytes.
    let mut chunks: Vec<js_sys::Uint8Array> = Vec::with_capacity(64);
    let mut bytes = 0usize;
    loop {
        let chunk = body.next_chunk().await.map_err(|e| {
            // Logged before the collected pieces are dropped: a truncated export
            // saved as though it were whole is worse than a failed download.
            log::error!(
                "event=download format={} phase=stream state=failed collected_bytes={bytes} error={e}",
                format.log_name()
            );
            e.to_string()
        })?;

        // `None` is the end of the body, which is how this loop ends. An empty
        // chunk is a keep-alive from the transport rather than the end, so it is
        // skipped rather than mistaken for a finished export.
        let Some(chunk) = chunk else { break };
        if chunk.is_empty() {
            continue;
        }

        bytes += chunk.len();
        chunks.push(js_sys::Uint8Array::from(&chunk[..]));
    }

    let fetch_elapsed = perf::end_timer(export_timer_label(format), dl_timer);
    perf::log_timing(
        "download",
        &format!(
            "event=download format={} phase=fetch state=success source=wdqs body_bytes={} chunks={}",
            format.log_name(),
            bytes,
            chunks.len()
        ),
        Some(fetch_elapsed),
    );

    // A query that matched nothing comes back as an empty body. That is a result,
    // so it is saved as an empty file rather than reported as a failure the reader
    // would have to interpret.
    if chunks.is_empty() {
        log::warn!(
            "event=download format={} phase=stream state=empty",
            format.log_name()
        );
    }

    // Determine the MIME type for the downloaded file
    let mime = wdqs_content_type(format);

    let trigger_timer = perf::start_timer(&export_trigger_timer_label(format));
    if let Err(e) = crate::upload::download_byte_chunks_as_blob(&chunks, &filename, "", mime) {
        log::error!("download failed: filename={filename} mime={mime} error={e}");
        return Err(e);
    }
    let trigger_elapsed = perf::end_timer(&export_trigger_timer_label(format), trigger_timer);
    perf::log_timing(
        "download",
        &format!(
            "event=download format={} phase=trigger state=success source=wdqs",
            format.log_name()
        ),
        Some(trigger_elapsed),
    );
    Ok(())
}

/// Returns the MIME type for downloaded file content.
fn wdqs_content_type(format: DownloadFormat) -> &'static str {
    match format {
        DownloadFormat::Csv => "text/csv",
        DownloadFormat::Json => "application/sparql-results+json",
        DownloadFormat::Rdf => "text/turtle",
    }
}

async fn execute_download_wasm_browser_post(
    format: DownloadFormat,
    query: Arc<str>,
    filename: String,
    dl_timer: perf::TimerHandle,
) -> Result<(), String> {
    let prepared_query = format.prepared_query(query.as_ref());
    let action = format.qlever_action().to_string();

    let fetch_elapsed = perf::end_timer(export_timer_label(format), dl_timer);
    perf::log_timing(
        "download",
        &format!(
            "event=download format={} phase=fetch state=delegated source=browser_post",
            format.log_name(),
        ),
        Some(fetch_elapsed),
    );

    let trigger_timer = perf::start_timer(&export_trigger_timer_label(format));
    crate::upload::submit_download_form(
        QLEVER_WIKIDATA,
        &[
            ("query", &prepared_query),
            ("action", &action),
            ("filename", &filename),
        ],
    )
    .await?;
    let trigger_elapsed = perf::end_timer(&export_trigger_timer_label(format), trigger_timer);
    perf::log_timing(
        "download",
        &format!(
            "event=download format={} phase=trigger state=success source=browser_post",
            format.log_name()
        ),
        Some(trigger_elapsed),
    );
    Ok(())
}

fn select_export_url(format: DownloadFormat, urls: &api::ExportUrlResponse) -> &str {
    match format {
        DownloadFormat::Csv => urls.csv_gz_url.as_deref().unwrap_or(&urls.csv_url),
        DownloadFormat::Json => urls.json_gz_url.as_deref().unwrap_or(&urls.json_url),
        DownloadFormat::Rdf => urls.rdf_gz_url.as_deref().unwrap_or(&urls.rdf_url),
    }
}

fn append_filename_query(url: &str, filename: &str) -> String {
    let sep = if url.contains('?') { '&' } else { '?' };
    format!("{url}{sep}filename={}", urlencoding::encode(filename))
}

pub(super) fn trigger_download(filename: &str, mime: &str, content_or_url: &str) {
    if content_or_url.starts_with("http://") || content_or_url.starts_with("https://") {
        let _ = crate::upload::download_url(content_or_url, filename);
        let _ = mime;
        return;
    }

    log::debug!(
        "download_text_as_blob filename={filename} mime={mime} content_len={}",
        content_or_url.len()
    );
    if let Err(e) = crate::upload::download_text_as_blob(content_or_url, filename, "", mime) {
        log::error!("download failed: filename={filename} mime={mime} error={e}");
    }
}
