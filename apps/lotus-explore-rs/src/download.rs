// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Shared download helpers for browser/native targets, including format handling & deduplication.

pub use lotus::export::ExportFormat as DownloadFormat;

use std::sync::Arc;

use crate::perf;

#[cfg(target_arch = "wasm32")]
use crate::models::SearchCriteria;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod wasm;

/// Execute a download in the given format.
pub async fn execute_download(
    format: DownloadFormat,
    #[cfg(target_arch = "wasm32")] criteria: std::sync::Arc<SearchCriteria>,
    query: Arc<str>,
    filename: String,
) -> Result<(), String> {
    let dl_timer = perf::start_timer(format.timer_label());
    log::info!("event=download format={} state=started", format.log_name());

    #[cfg(target_arch = "wasm32")]
    {
        wasm::execute_download_wasm(format, criteria, query, filename, dl_timer).await
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        native::execute_download_with_fallback(format, query, filename, dl_timer).await
    }
}

pub fn trigger_download(filename: &str, mime: &str, content_or_url: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        wasm::trigger_download(filename, mime, content_or_url);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        native::trigger_download(filename, mime, content_or_url);
    }
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
