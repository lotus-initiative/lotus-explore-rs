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
    format!(
        "this browser cannot write a file directly, so an export of {rows} rows has to be \
         assembled in memory, which is more than it can hold (the limit here is {limit} \
         rows). A current version of Safari, Firefox or Chrome writes the file to disk \
         instead and has no such limit."
    )
}

/// The largest export the in-memory path will attempt.
///
/// There is no way to make assembling a file in memory not exhaust a tab; the only
/// question is whether it fails as an error a reader can read or as a killed tab. On the
/// two writable paths this is never reached, because nothing is accumulated.
///
/// 400,000 rows is roughly 100 MB of CSV. That is past the point where holding it stops
/// being the dominant term on a phone, and low enough that the refusal lands before the
/// tab does -- a full-size search is three million rows and about 600 MB, so the ceiling
/// sits an order of magnitude below it and three orders above an ordinary search. A
/// reader over the limit is told what to do rather than left to reload.
pub const BLOB_PATH_ROW_LIMIT: usize = 400_000;

/// Whether the in-memory path can carry `rows`.
///
/// Checked before a single chunk is produced, so finding out costs one comparison
/// rather than a killed tab.
#[must_use]
pub fn blob_path_can_carry(rows: usize) -> bool {
    rows <= BLOB_PATH_ROW_LIMIT
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

#[cfg(test)]
mod sink_tests {
    use super::{SINK_PREFERENCE, SinkPreference};

    #[test]
    fn memory_is_the_last_resort() {
        // `Blob` is the variant that assembles the whole export in the tab's heap, and
        // that is what crashed the tab at full size. It must be reachable only when
        // neither file sink is.
        assert_eq!(
            SINK_PREFERENCE.last(),
            Some(&SinkPreference::Memory),
            "the in-memory sink must be the final fallback, not the first choice"
        );
    }

    #[test]
    fn the_reader_chosen_file_is_preferred_over_private_storage() {
        // Where both exist the reader picks the location and no temporary file is
        // involved. Falling through to OPFS on Chromium would add a temp file and a
        // delete for no gain.
        assert_eq!(SINK_PREFERENCE[0], SinkPreference::UserChosenFile);
        assert!(
            SINK_PREFERENCE
                .iter()
                .position(|s| *s == SinkPreference::PrivateStorage)
                < SINK_PREFERENCE
                    .iter()
                    .position(|s| *s == SinkPreference::Memory),
            "private storage must be tried before falling back to memory"
        );
    }

    #[test]
    fn every_preference_is_distinct() {
        let mut seen = SINK_PREFERENCE.to_vec();
        seen.sort_by_key(|s| format!("{s:?}"));
        seen.dedup();
        assert_eq!(
            seen.len(),
            SINK_PREFERENCE.len(),
            "a duplicated preference means one branch can never be reached"
        );
    }
}

#[cfg(test)]
mod blob_limit_tests {
    //! The in-memory fallback has a row ceiling, and this is the policy behind it.
    //!
    //! It lives here rather than in the wasm-only download module for the reason the
    //! sink order does: a policy whose tests never run is not a policy.

    #[test]
    fn the_in_memory_path_refuses_past_the_ceiling() {
        // The failure this prevents is a killed tab, not an error message.
        assert!(super::blob_path_can_carry(0));
        assert!(super::blob_path_can_carry(10_000));
        assert!(
            super::blob_path_can_carry(super::BLOB_PATH_ROW_LIMIT),
            "the ceiling itself must be allowed"
        );
        assert!(
            !super::blob_path_can_carry(super::BLOB_PATH_ROW_LIMIT + 1),
            "one row past the ceiling must be refused"
        );
    }

    #[test]
    fn the_message_names_the_browser_limit_and_the_way_out() {
        let limit = super::BLOB_PATH_ROW_LIMIT;
        let message = super::blob_path_message(limit + 1, limit);
        assert!(message.contains(&format!("{limit}")), "{message}");
        assert!(
            message.contains("disk"),
            "a reader who cannot act on the limit needs to be told what would lift it: \
             {message}"
        );
    }
}

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
mod opfs_availability_gate {
    /// The wasm-only module, as text. Compiled here only to be read.
    const SOURCE: &str = include_str!("download/file_sink.rs");

    #[test]
    fn availability_is_read_from_navigator_not_window() {
        assert!(
            !SOURCE.contains(r#"Reflect::get(&window, &"storage""#),
            "there is no `window.storage`; the origin private file system is \
             `navigator.storage`, so this reads a property that does not exist"
        );
        assert!(
            SOURCE.contains("navigator().storage()"),
            "OPFS availability should be read from `navigator().storage()`"
        );
    }

    #[test]
    fn presence_is_not_tested_by_asking_whether_fetching_threw() {
        // `Reflect::get` reports a missing property as `Ok(undefined)`, so
        // `.is_ok()` on its result is true for a browser that has never heard of
        // OPFS. Both the check and the lookup have to test for a function.
        assert!(
            !SOURCE.contains(r#""getDirectory".into()).is_ok()"#),
            "`Reflect::get` returns Ok for a missing property, so this claims \
             OPFS exists wherever it does not"
        );
        // Counted over code lines, and deliberately not by naming the two
        // expressions. Naming them was the previous spelling and it broke on a
        // `clippy::redundant_closure` fix that changed one of them from
        // `|value| value.is_function()` to `JsValue::is_function` without
        // changing what the code means -- a source gate that fails when a lint
        // rewrites a closure is a gate that gets deleted rather than fixed.
        //
        // Comments are excluded because the module's own doc explains why
        // `is_function` is there, and a count that includes the explanation
        // fails whenever the explanation is reworded.
        let code: String = SOURCE
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let checks = code.matches("is_function").count();
        assert!(
            checks >= 2,
            "both the availability check and the directory lookup have to test \
             for a function rather than for the absence of an error; found \
             {checks} in code"
        );
    }
}
