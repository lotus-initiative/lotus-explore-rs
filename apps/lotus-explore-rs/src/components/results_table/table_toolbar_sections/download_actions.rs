// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Download toolbar group: buttons that trigger query/metadata downloads.

use super::super::download_model::{
    DOWNLOAD_METADATA_SPEC, DOWNLOAD_QUERY_CSV_SPEC, DOWNLOAD_QUERY_JSON_SPEC,
    DOWNLOAD_QUERY_RDF_SPEC, DownloadQuerySpec, SparqlEndpointUI,
    build_download_toolbar_model_with_endpoint,
};
use crate::components::ui::Button;
use crate::download::{execute_download, trigger_download};
use crate::features::explore::use_toolbar_result_snapshot;
use crate::i18n::{TextKey, t};
use crate::perf;
use crate::state::use_results_context;
use dioxus::prelude::*;
use lotus_query::ExportFormat as DownloadFormat;
use lotus_search::SearchCriteria;
use std::sync::Arc;

// `criteria_snapshot` is used on WASM only (threaded into the download
// there); unused on native, where the parameter exists for signature parity.
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(unused_variables, clippy::needless_pass_by_value)
)]
/// The three signals the download buttons share.
///
/// Grouped because passing them separately ran the argument count past the limit
/// clippy enforces, and because they are one thing: what the download UI is
/// currently showing.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct DownloadSignals {
    /// A download is in flight.
    pub busy: Signal<bool>,
    /// The in-flight message, shown next to a spinner.
    pub status: Signal<Option<String>>,
    /// The result of the last finished download, shown after the spinner is gone.
    pub notice: Signal<Option<String>>,
}

// The criteria snapshot is only needed by the browser build, which has to
// rebuild the query from the form the user submitted. A desktop build already
// holds the SPARQL string, so the snapshot is never read there.
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(unused_variables, clippy::needless_pass_by_value)
)]
fn spawn_query_download(
    format: DownloadFormat,
    status_message: String,
    criteria_snapshot: Option<Arc<SearchCriteria>>,
    filename: String,
    query: Arc<str>,
    signals: DownloadSignals,
    rows: Arc<lotus_model::ColumnarResultSet>,
) {
    let DownloadSignals {
        busy: download_busy,
        status: download_status,
        notice: download_notice,
    } = signals;
    let mut download_busy = download_busy;
    let mut download_status = download_status;
    let mut download_notice = download_notice;
    *download_busy.write() = true;
    *download_status.write() = Some(status_message);
    spawn(async move {
        log::info!(
            "event=download phase=table_dispatch state=started format={}",
            format.log_name()
        );
        log::info!(
            "event=download phase=table_query state=check format={} has_SERVICE={} query_bytes={}",
            format.log_name(),
            query.contains("SERVICE"),
            query.len()
        );
        let outcome: Result<String, String> = execute_download(
            format,
            #[cfg(target_arch = "wasm32")]
            {
                let Some(criteria_snapshot) = criteria_snapshot else {
                    log::warn!(
                        "event=download phase=table_query state=error reason=missing_criteria_snapshot"
                    );
                    // The picker was armed above and no export is going to consume it.
                    // Left armed it would sit on a handle the reader has already chosen
                    // a location for, and the next click would overwrite it -- so the
                    // file they picked would never appear.
                    crate::download::clear_file_sink();
                    *download_busy.write() = false;
                    *download_status.write() = None;
                    return;
                };
                criteria_snapshot
            },
            query,
            filename,
            // The rows the table is drawn from, so the file is written from them rather
            // than by asking a service to run the query again. Browser-only: a desktop
            // build has no route that builds a file in memory.
            #[cfg(target_arch = "wasm32")]
            Some(Arc::clone(&rows)),
        )
        .await;

        match &outcome {
            Ok(_) => log::info!(
                "event=download format={} phase=table_fetch state=success",
                format.log_name()
            ),
            Err(err) => log::warn!(
                "event=download format={} phase=table_fetch state=error reason={err}",
                format.log_name()
            ),
        }
        // On a desktop build the message is the path the file was written to. A
        // window has no download shelf, so without this the file appears in
        // ~/Downloads and the UI looks like the click did nothing.
        *download_notice.write() = Some(outcome.unwrap_or_else(|e| e));
        *download_busy.write() = false;
        *download_status.write() = None;
    });
}

fn dispatch_query_download_spec(
    spec: DownloadQuerySpec,
    locale: crate::i18n::Locale,
    criteria_snapshot: Option<Arc<SearchCriteria>>,
    filename: String,
    query: Arc<str>,
    signals: DownloadSignals,
    rows: Arc<lotus_model::ColumnarResultSet>,
) {
    spawn_query_download(
        spec.format,
        t(locale, spec.status_key).to_string(),
        criteria_snapshot,
        filename,
        query,
        signals,
        rows,
    );
}

fn dispatch_metadata_download_blob(
    filename: &str,
    body: &str,
    mut download_notice: Signal<Option<String>>,
) {
    log::info!(
        "event=download phase=table_dispatch state=started format=metadata filename={} size={}",
        filename,
        body.len()
    );
    let trigger_timer = perf::start_timer("LOTUS:table_download_meta_trigger");
    if body.is_empty() {
        log::error!(
            "event=download phase=table_dispatch state=error format=metadata reason=empty_body"
        );
        return;
    }
    let outcome = trigger_download(filename, "application/ld+json", body);
    *download_notice.write() = Some(match &outcome {
        Ok(()) => filename.to_string(),
        Err(e) => e.clone(),
    });
    if let Err(e) = outcome {
        log::error!("event=download phase=table_dispatch state=error format=metadata reason={e}");
        return;
    }
    let elapsed_ms =
        perf::end_timer("LOTUS:table_download_meta_trigger", trigger_timer).as_secs_f64() * 1000.0;
    log::info!(
        "event=download phase=table_trigger state=success format=metadata elapsed_ms={elapsed_ms:.1}"
    );
}

/// The result of a finished download: where the file went, or why it did not.
///
/// Separate from the spinner, which is only on screen while a download is in
/// flight. A desktop export finishes by writing a file, and without this the
/// user has no way to tell that from a click that did nothing.
#[component]
fn DownloadNotice(notice: ReadSignal<Option<String>>) -> Element {
    let Some(text) = notice.read().clone() else {
        return rsx! {};
    };
    rsx! {
        span {
            role: "status",
            aria_live: "polite",
            class: "inline-flex items-center gap-2 rounded-xl border border-border bg-surface px-3 py-1.5 text-ui font-semibold text-muted shadow-xs",
            "{text}"
        }
    }
}

/// Displays download status with spinning indicator.
#[component]
fn DownloadStatusSpinner(
    download_status: ReadSignal<Option<String>>,
    locale: crate::i18n::Locale,
) -> Element {
    let status_msg = download_status.read().clone();
    let text = status_msg
        .as_deref()
        .unwrap_or_else(|| t(locale, TextKey::PreparingDownload));

    rsx! {
        span {
            role: "status",
            aria_live: "polite",
            class: "inline-flex items-center gap-2 rounded-xl border border-border bg-surface px-3 py-1.5 text-ui font-semibold text-muted shadow-xs",
            span { class: "spinner-sm", "aria-hidden": "true" }
            {text}
        }
    }
}

/// Download button for query results (CSV, JSON, RDF formats).
#[component]
fn DownloadQueryButton(
    spec: DownloadQuerySpec,
    sparql_query: Arc<str>,
    locale: crate::i18n::Locale,
    disabled: bool,
    signals: DownloadSignals,
    criteria: ReadSignal<SearchCriteria>,
    /// The rows on screen. The export is written from these rather than by asking a
    /// service to run the query a second time.
    ///
    /// An `ArcPtrEq` memo rather than a plain signal: the set is behind an `Arc` and
    /// comparing two sets by value would walk every row to decide whether the toolbar
    /// should re-render. Pointer equality is both correct -- a new set is a new
    /// pointer -- and free.
    rows: ReadSignal<
        crate::features::explore::selectors::ArcPtrEq<lotus_model::ColumnarResultSet>,
    >,
    filename: String,
) -> Element {
    let title = t(locale, spec.title_key);
    let label = t(locale, spec.label_key);

    rsx! {
        Button {
            r#type: "button",
            disabled,
            class: "inline-flex shrink-0 items-center justify-center font-sans select-none transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 rounded-xl border border-border bg-surface text-text font-semibold shadow-xs hover:bg-bg active:bg-bg min-h-9 px-4 py-1.5 text-ui active:scale-[0.98]",
            title: Some(title.to_string()),
            // WCAG 2.5.3: the accessible name must contain the visible label.
            // The tooltip carries the longer description instead.
            label: Some(label.to_string()),
            onclick: {
                let filename = move || filename.clone();
                // Read inside the handler, not here. The `onclick: { .. }` block
                // is evaluated while the template is built, so hoisting the
                // snapshot out of the closure deep-copied a `SearchCriteria`
                // (three `String`s) on every render of the toolbar — three
                // times over, once for each of the CSV/JSON/RDF buttons — with
                // no click involved. Reading at click time is also the fresher
                // value: it is the criteria of the results on screen now.
                move |_| {
                    #[cfg(target_arch = "wasm32")]
                    let criteria_snapshot = Some(Arc::new(criteria.read().clone()));
                    #[cfg(not(target_arch = "wasm32"))]
                    let criteria_snapshot = None;

                    // Here, inside the handler, and nowhere later: `showSaveFilePicker`
                    // is only permitted while the click is still a user gesture, and
                    // the export runs in a task `spawn`ed from this handler, which has
                    // already lost it by the time it asks. Arming it now and letting the
                    // export collect the handle is what lets a full-size file be
                    // written to disk instead of assembled in memory.
                    //
                    // The suggested name is the one the button already displays, so the
                    // dialog opens on the filename the reader was promised.
                    #[cfg(target_arch = "wasm32")]
                    {
                        crate::download::arm_file_sink(&filename());
                    }
                    dispatch_query_download_spec(
                        spec,
                        locale,
                        criteria_snapshot,
                        filename(),
                        sparql_query.clone(),
                        signals,
                        // The rows on screen. Read once here so the file can be written
                        // from them instead of re-running the query elsewhere.
                        Arc::clone(&rows.read().0),
                    );
                }
            },
        }
    }
}

/// Download button for metadata JSON file.
#[component]
fn DownloadMetadataButton(
    metadata_json: Arc<str>,
    toolbar_model: ReadSignal<
        crate::components::results_table::download_model::DownloadToolbarModel,
    >,
    locale: crate::i18n::Locale,
    disabled: bool,
    download_notice: Signal<Option<String>>,
) -> Element {
    let title = t(locale, DOWNLOAD_METADATA_SPEC.title_key);
    let label = t(locale, DOWNLOAD_METADATA_SPEC.label_key);

    rsx! {
        Button {
            r#type: "button",
            disabled,
            class: "inline-flex shrink-0 items-center justify-center font-sans select-none transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 rounded-xl border border-border bg-surface text-text font-semibold shadow-xs hover:bg-bg active:bg-bg min-h-9 px-4 py-1.5 text-ui active:scale-[0.98]",
            title: Some(title.to_string()),
            // See the note in download_query_button: the visible label is the
            // accessible name, the tooltip holds the description.
            label: Some(label.to_string()),
            onclick: {
                // Same reasoning as `DownloadQueryButton`: this block runs
                // while the template is built, so the filename was rebuilt on
                // every render rather than on the click that needs it.
                move |_| {
                    let filename = toolbar_model.read().metadata_filename.clone();
                    dispatch_metadata_download_blob(
                        &filename,
                        metadata_json.as_ref(),
                        download_notice,
                    );
                }
            },
        }
    }
}

#[component]
pub fn DownloadActionsGroup() -> Element {
    let locale = crate::hooks::use_locale();
    let explore = use_results_context().explore;

    // Each selector subscribes to exactly one field; the component only
    // re-renders when any of these specific fields change.
    let criteria = crate::features::explore::selectors::use_ui_selector(explore, |ui| {
        ui.executed_criteria.clone()
    });
    // Subscribed separately from `criteria` so the toolbar re-renders when the rows
    // change, not only when the criteria do. Same reasoning as the other selectors.
    let rows = crate::features::explore::selectors::use_result_arc_selector(explore, |result| {
        result.set.clone()
    });
    let toolbar_snapshot = use_toolbar_result_snapshot(explore);

    let snapshot = toolbar_snapshot.read();
    let toolbar_model = use_signal(|| {
        build_download_toolbar_model_with_endpoint(
            &criteria.read(),
            snapshot.sparql_query.as_deref(),
            snapshot.metadata_json.as_deref(),
            snapshot.query_hash.as_deref(),
            snapshot.result_hash.as_deref(),
            snapshot.endpoint.into(),
        )
    });

    let download_results_label = t(locale, TextKey::DownloadResults);

    // Separate `notice` from `status`, which the spinner shows only while a
    // download is in flight. A desktop export finishes by writing a file, and
    // nothing on screen would otherwise say so.
    let signals = DownloadSignals {
        busy: use_signal(|| false),
        status: use_signal(|| None),
        notice: use_signal(|| None),
    };

    let sparql_query_value = snapshot.sparql_query.clone();
    let metadata_json_value = snapshot.metadata_json.clone();
    let toolbar = toolbar_model.read();
    let export_available = toolbar.export_available;
    let ui_url_for_click = toolbar.ui_url.clone();
    let endpoint_name = toolbar.sparql_endpoint_ui.to_string();
    let (open_in_label, open_in_title) = match toolbar.sparql_endpoint_ui {
        SparqlEndpointUI::Qlever => (
            t(locale, TextKey::OpenInQlever),
            t(locale, TextKey::OpenInQleverTitle),
        ),
        SparqlEndpointUI::Wdqs => (
            t(locale, TextKey::OpenInEndpoint),
            t(locale, TextKey::OpenInEndpointTitle),
        ),
    };
    drop(snapshot);
    drop(toolbar);

    rsx! {
        nav { class: "flex w-full min-w-0 flex-wrap items-center justify-center gap-3 py-1 mb-3", aria_label: "{download_results_label}",
            if *signals.busy.read() {
                DownloadStatusSpinner {
                    download_status: signals.status,
                    locale,
                }
            } else if signals.notice.read().is_some() {
                li {
                    DownloadNotice { notice: signals.notice }
                }
            }
            if export_available {
                ul {
                    class: "flex min-w-0 flex-wrap items-center justify-center gap-3",
                    if let Some(query) = sparql_query_value.as_ref() {
                        li {
                            DownloadQueryButton {
                                spec: DOWNLOAD_QUERY_CSV_SPEC,
                                sparql_query: query.clone(),
                                locale,
                                disabled: *signals.busy.read(),
                                signals,
                                criteria,
                                rows: ReadSignal::from(rows),
                                filename: toolbar_model.read().csv_filename.clone(),
                            }
                        }
                        li {
                            DownloadQueryButton {
                                spec: DOWNLOAD_QUERY_JSON_SPEC,
                                sparql_query: query.clone(),
                                locale,
                                disabled: *signals.busy.read(),
                                signals,
                                criteria,
                                rows: ReadSignal::from(rows),
                                filename: toolbar_model.read().json_filename.clone(),
                            }
                        }
                        li {
                            DownloadQueryButton {
                                spec: DOWNLOAD_QUERY_RDF_SPEC,
                                sparql_query: query.clone(),
                                locale,
                                disabled: *signals.busy.read(),
                                signals,
                                criteria,
                                rows: ReadSignal::from(rows),
                                filename: toolbar_model.read().rdf_filename.clone(),
                            }
                        }
                    }
                    if let Some(body) = metadata_json_value.as_ref() {
                        li {
                            DownloadMetadataButton {
                                metadata_json: body.clone(),
                                toolbar_model,
                                locale,
                                disabled: *signals.busy.read(),
                                download_notice: signals.notice,
                            }
                        }
                    }
                    if let Some(_url) = ui_url_for_click.as_ref() {
                        li {
                            Button {
                                r#type: "button",
                                class: "inline-flex shrink-0 items-center justify-center font-sans select-none transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 rounded-xl border border-border bg-surface text-text font-semibold shadow-xs hover:bg-bg active:bg-bg min-h-9 px-4 py-1.5 text-ui active:scale-[0.98]",
                                title: Some(format!("{open_in_title} ({endpoint_name})")),
                                label: Some(open_in_label.to_string()),
                                onclick: move |_| {
                                    let Some(url) = ui_url_for_click.as_ref() else {
                                        return;
                                    };
                                    #[cfg(target_arch = "wasm32")]
                                    if let Some(win) = web_sys::window() {
                                        let _ = win.open_with_url_and_target(url, "_blank");
                                    }
                                    // A desktop window is not a browser, so
                                    // there is nothing to navigate. This branch
                                    // used to be absent, which made the button a
                                    // no-op in the window.
                                    #[cfg(not(target_arch = "wasm32"))]
                                    if let Err(e) = crate::download::open_externally(url) {
                                        log::warn!(
                                            "event=open_external state=error reason={e}"
                                        );
                                    }
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}
