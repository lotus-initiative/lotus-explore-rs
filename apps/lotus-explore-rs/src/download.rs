// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Shared download helpers for browser/native targets, including format handling & deduplication.

use lotus_query::ExportFormat as DownloadFormat;

use std::sync::Arc;

use crate::perf;

#[cfg(target_arch = "wasm32")]
use lotus_search::SearchCriteria;

#[cfg(not(target_arch = "wasm32"))]
mod local_file;

#[cfg(not(target_arch = "wasm32"))]
pub use local_file::open_externally;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod wasm;

/// Execute a download in the given format.
///
/// Returns a message describing the outcome, for display. On a desktop build that
/// is the path the file was written to: the window has no download shelf, so a
/// file appearing in `~/Downloads` with nothing on screen is indistinguishable
/// from a button that did nothing.
pub async fn execute_download(
    format: DownloadFormat,
    #[cfg(target_arch = "wasm32")] criteria: std::sync::Arc<SearchCriteria>,
    query: Arc<str>,
    filename: String,
) -> Result<String, String> {
    let dl_timer = perf::start_timer(export_timer_label(format));
    log::info!("event=download format={} state=started", format.log_name());

    #[cfg(target_arch = "wasm32")]
    {
        let name = filename.clone();
        wasm::execute_download_wasm(format, criteria, query, filename, dl_timer)
            .await
            // The browser owns the download and shows its own progress UI, so
            // there is nothing for the app to add.
            .map(|()| name)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        native::execute_download_with_fallback(format, query, filename, dl_timer).await
    }
}

/// Hand `content_or_url` to the user.
///
/// # Errors
/// On a desktop build, where the payload is written to a file: a message if the
/// file could not be written. The browser build cannot fail -- the browser owns
/// the download and shows its own UI -- so the signature is uniform and the
/// browser arm is allowed to be infallible.
#[cfg_attr(target_arch = "wasm32", allow(clippy::unnecessary_wraps))]
pub fn trigger_download(filename: &str, mime: &str, content_or_url: &str) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        wasm::trigger_download(filename, mime, content_or_url);
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        native::trigger_download(filename, mime, content_or_url).map(|_| ())
    }
}

/// `perf` label for a download in this format.
///
/// A label rather than the format's own name because the perf panel groups by
/// timer, and `"LOTUS:download_csv"` is what the existing dashboards key on.
#[must_use]
pub fn export_timer_label(format: lotus_query::ExportFormat) -> &'static str {
    match format {
        lotus_query::ExportFormat::Csv => "LOTUS:download_csv",
        lotus_query::ExportFormat::Json => "LOTUS:download_json",
        lotus_query::ExportFormat::Rdf => "LOTUS:download_rdf",
    }
}

/// `perf` label for the click that started a download in this format.
///
/// A separate measurement from the download itself: a slow download and a slow
/// render are different problems, and one timer cannot distinguish them.
#[must_use]
pub fn export_trigger_timer_label(format: lotus_query::ExportFormat) -> String {
    format!("{}_trigger", export_timer_label(format))
}

/// The response format to ask `WDQS` for when downloading this export.
///
/// The same choice seen from the transport's end.
pub fn wdqs_response_format(format: lotus_query::ExportFormat) -> lotus_search::ResponseFormat {
    format.into()
}

#[cfg(test)]
mod tests {
    use super::DownloadFormat;

    #[test]
    fn parse_download_format_supports_documented_aliases() {
        assert_eq!(DownloadFormat::parse("csv"), Some(DownloadFormat::Csv));
        assert_eq!(DownloadFormat::parse("json"), Some(DownloadFormat::Json));
        assert_eq!(DownloadFormat::parse("ndjson"), Some(DownloadFormat::Json));
        assert_eq!(DownloadFormat::parse("rdf"), Some(DownloadFormat::Rdf));
        assert_eq!(DownloadFormat::parse(" JSON "), Some(DownloadFormat::Json));
        assert_eq!(DownloadFormat::parse("RDF"), Some(DownloadFormat::Rdf));
        assert_eq!(DownloadFormat::parse("ttl"), None);
    }
}

#[cfg(test)]
mod streaming_gate {
    //! The download must not go back to buffering the whole export.
    //!
    //! A source-level check rather than a behavioural one, because the code it
    //! guards is `wasm`-only and cannot be exercised by a native test run. What it
    //! pins is the *call*: the WDQS path has to read the body through
    //! `execute_sparql_chunks_at`, which streams, and must not reach for
    //! `execute_sparql_format_at`, which returns the entire export as one `String`.
    //!
    //! That difference is invisible until it is fatal. A two-million-row export is
    //! roughly 600 MB of CSV; decoded into a `String` in a tab budgeted for 200 MB
    //! it is a killed tab with no error, and the download button simply does
    //! nothing on a large result.
    //!
    //! The identifier is matched *with* its parenthesis, so a comment explaining
    //! why the buffering call is gone does not trip the gate.

    /// The wasm download module, as text. Compiled here only to be read.
    const WASM_DOWNLOAD: &str = include_str!("download/wasm.rs");

    #[test]
    fn the_wdqs_download_streams_rather_than_buffering() {
        assert!(
            WASM_DOWNLOAD.contains("execute_sparql_chunks_at"),
            "the WDQS download should read the body through the chunked reader"
        );
        assert!(
            !WASM_DOWNLOAD.contains("execute_sparql_format_at("),
            "the WDQS download must not buffer the whole export into one String: \
             that is what this path was rewritten to stop doing"
        );
    }

    #[test]
    fn a_streamed_export_is_handed_over_as_its_parts() {
        assert!(
            WASM_DOWNLOAD.contains("download_byte_chunks_as_blob"),
            "the collected chunks are what gets saved, not a decoded string"
        );
    }
}
