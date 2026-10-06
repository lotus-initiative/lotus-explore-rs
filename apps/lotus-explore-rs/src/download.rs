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
#[cfg(target_arch = "wasm32")]
mod file_sink;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
pub use file_sink::arm_file_sink;
#[cfg(target_arch = "wasm32")]
pub use file_sink::clear_file_sink;
#[cfg(target_arch = "wasm32")]
mod wasm;

/// Which sink a given browser gets.
///
/// The order is load-bearing: [`SinkPreference::Memory`] holds the entire export in the
/// tab's heap, which is the case that crashed it, so it must never be reachable while
/// either file sink is available. Declared here rather than in the wasm-only module
/// because it is a policy rather than an implementation -- and a policy in a
/// `cfg(target_arch = "wasm32")` module is a policy whose tests never run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SinkPreference {
    /// Chromium: `showSaveFilePicker`, so the reader chooses the location.
    UserChosenFile,
    /// Safari 15.2+ and Firefox 111+: the origin private file system.
    PrivateStorage,
    /// No writable sink; assembled in memory.
    Memory,
}

/// The preference order, best first.
pub const SINK_PREFERENCE: [SinkPreference; 3] = [
    SinkPreference::UserChosenFile,
    SinkPreference::PrivateStorage,
    SinkPreference::Memory,
];

/// What to tell a reader whose browser can write the file nowhere.
pub fn blob_path_message(rows: usize, limit: usize) -> String {
    let budget = IN_MEMORY_BUDGET_BYTES / (1024 * 1024);
    format!(
        "this browser cannot write a file directly, so an export of {rows} rows has to be \
         assembled in memory, which is more than it can hold (the limit here is {limit} \
         rows for this format, against a {budget} MB budget; CSV holds a million and \
         Turtle fewer, because the formats are not the same size). A current version of \
         Safari, Firefox or Chrome writes the file to disk instead and has no such \
         limit."
    )
}

/// How much RAM assembling one row costs in each format.
///
/// **Measured, not guessed.** Produced by `cargo run -p lotus-query --example
/// measure_export --release`, which drives the real [`RowExporter`] over a
/// 200,000-row set built from the recorded payload shape and counts the bytes it
/// emits. Reproduce with that command; the example is the provenance for every
/// number here.
///
/// These replace structural estimates that had no evidence behind them, and two of
/// the three were badly wrong in the direction that costs a reader their export:
///
/// | format | was | measured | error |
/// |--------|-----|----------|-------|
/// | CSV    | 471  | 116      | 4.1x too high |
/// | JSON   | 700  | 667      | about right |
/// | RDF    | 1500 | 276      | 5.4x too high |
///
/// Turtle was the worst and the most expensive to get wrong, because it is the
/// format with the lowest ceiling and therefore the one a reader meets first. The
/// old comment guessed "~8 triples per row, namespaces repeated" and landed at
/// five times the truth: the emitter writes the same handful of IRIs per row, and
/// an entity QID is short. A refused 708,200-row Turtle export was 195 MB of
/// real bytes, not the 1.06 GB the old constant claimed.
///
/// Ordered by a test below, because an estimate that reorders would make the gate
/// wrong in the direction nobody notices.
const BYTES_PER_ROW: &[(DownloadFormat, usize)] = &[
    // 200,000 rows -> 23,191,983 bytes
    (DownloadFormat::Csv, 116),
    // 200,000 rows -> 133,392,052 bytes
    (DownloadFormat::Json, 667),
    // 200,000 rows -> 55,151,433 bytes
    (DownloadFormat::Rdf, 276),
];

/// The RAM the in-memory path may assemble before the tab goes.
///
/// 512 MB, and this number is a judgement rather than a measurement -- it is the
/// one constant here with no measurement behind it, and it is worth saying so
/// rather than letting it read like the other two.
///
/// The reasoning is that this path holds the finished file in the tab's heap next
/// to a result set already most of the tab's budget, and the failure is a killed
/// tab rather than an error message. It is raised from 512 MB only if that
/// reasoning changes; nothing here measures a browser's heap.
///
/// Reached only where nothing can stream: no save picker and no private storage,
/// which is an outdated engine, and some private-browsing modes. Every current
/// browser takes one of the two streaming paths instead, where this budget does
/// not apply because no chunk is ever retained. A million rows of any format pass
/// on those, and a million rows of Turtle is 276 MB written to disk.
const IN_MEMORY_BUDGET_BYTES: usize = 512 * 1024 * 1024;

/// The most rows the in-memory path will attempt, in `format`.
#[must_use]
pub fn blob_path_ceiling(format: DownloadFormat) -> usize {
    let per_row = BYTES_PER_ROW
        .iter()
        .find(|(f, _)| *f == format)
        .map_or(CSV_BYTES_PER_ROW, |(_, b)| *b);
    let per_row = per_row.max(1);
    (IN_MEMORY_BUDGET_BYTES / per_row).min(DOWNLOAD_MAX_ROWS)
}

/// The per-row cost assumed for a format missing from [`BYTES_PER_ROW`].
///
/// The CSV measurement, because CSV is the format whose rows are narrowest and so
/// the least damaging number to assume for a format nobody has measured.
const CSV_BYTES_PER_ROW: usize = 116;

/// The row ceiling a streaming export reaches, and where the fallback tops out for
/// CSV. A million CSV rows is 471 MB, which is what makes it the round number: the
/// budget buys it, and the next format up does not fit in it.
pub const DOWNLOAD_MAX_ROWS: usize = 1_000_000;

/// Whether the in-memory path can carry `rows` of `format`.
///
/// Checked before a single chunk is produced, so finding out costs one comparison
/// rather than a killed tab.
#[must_use]
pub fn blob_path_can_carry(rows: usize, format: DownloadFormat) -> bool {
    rows <= blob_path_ceiling(format)
}

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
    #[cfg(target_arch = "wasm32")] rows: Option<std::sync::Arc<lotus_model::ColumnarResultSet>>,
) -> Result<String, String> {
    let dl_timer = perf::start_timer(export_timer_label(format));
    log::info!("event=download format={} state=started", format.log_name());

    // Built here, from the rows already in memory, before anything is asked of
    // anybody else.
    //
    // This is the route that produces the filename the reader was shown. Handing the
    // export to `QLever` or the API means that service names the file as well, and it
    // does not name it the way the toolbar said it would -- so the name on screen and
    // the name on disk disagreed, with nothing in between able to reconcile them.
    // It also means re-running a query the tab had already run to get the same rows.
    //
    // `rows` is `None` when there is no result set to read, which is the reference-lookup
    // and metadata paths: those have no row set of their own. Those still go out to a
    // service, because there is nothing here to build from.
    #[cfg(target_arch = "wasm32")]
    if let Some(set) = rows {
        return wasm::execute_download_from_rows(format, &set, &filename, dl_timer)
            .await
            .map(|()| filename);
    }

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
#[path = "download/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "download/streaming_gate.rs"]
mod streaming_gate;

#[cfg(test)]
#[path = "download/sink_tests.rs"]
mod sink_tests;

#[cfg(test)]
#[path = "download/blob_limit_tests.rs"]
mod blob_limit_tests;

/// A gate on the OPFS availability check, which no behavioural test can reach.
///
/// The check lives in a `wasm32`-only module, so a test beside it would never
/// run -- and the bug it guards was invisible to a type checker and to review,
/// because `Reflect::get(...).is_ok()` compiles and reads like a presence test.
/// It is not one: reading an absent property returns `Ok`, so the old expression
/// was a tautology. That claimed OPFS existed everywhere, and every export then
/// failed to open it and fell back to the in-memory path below, which is how an
/// iPhone ended up refusing a million rows at a ceiling meant for browsers that
/// cannot write to disk at all.
///
/// So this reads the source, for the same reason `SinkPreference` lives here.
///
/// A check that cannot fail is worse than no check, which is why the shape of it
/// is written down rather than left to the next reader.
#[cfg(test)]
#[path = "download/opfs_availability_gate.rs"]
mod opfs_availability_gate;

#[cfg(test)]
#[path = "download/opfs_write_stream.rs"]
mod opfs_write_stream;
