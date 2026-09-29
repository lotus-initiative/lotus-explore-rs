// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Shared download helpers for browser/native targets, including format handling & deduplication.

use lotus_query::ExportFormat as DownloadFormat;

use std::sync::Arc;

use crate::perf;

#[cfg(target_arch = "wasm32")]
use lotus_search::SearchCriteria;

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

/// `perf` label for a download of this format.
///
/// A label rather than the format's own name because the perf panel groups by
/// timer, and `"LOTUS:download_csv"` is what the existing dashboards key on.
pub trait ExportTimerLabel {
    /// The timer label for the download itself.
    fn timer_label(&self) -> &'static str;
    /// The timer label for the click that started it, which is a separate
    /// measurement: a slow download and a slow render are different problems.
    fn trigger_timer_label(&self) -> String;
}

impl ExportTimerLabel for lotus_query::ExportFormat {
    fn timer_label(&self) -> &'static str {
        match self {
            Self::Csv => "LOTUS:download_csv",
            Self::Json => "LOTUS:download_json",
            Self::Rdf => "LOTUS:download_rdf",
        }
    }

    fn trigger_timer_label(&self) -> String {
        format!("{}_trigger", self.timer_label())
    }
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
