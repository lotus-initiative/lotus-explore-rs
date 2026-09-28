// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Download actions toolbar group — buttons to trigger query/metadata downloads
//! and links to open the query in the `QLever` UI.

use super::super::download_model::{
    DOWNLOAD_METADATA_SPEC, DOWNLOAD_QUERY_CSV_SPEC, DOWNLOAD_QUERY_JSON_SPEC,
    DOWNLOAD_QUERY_RDF_SPEC, DownloadQuerySpec, SparqlEndpointUI,
    build_download_toolbar_model_with_endpoint,
};
use crate::components::ui::Button;
use crate::download::{DownloadFormat, execute_download, trigger_download};
use crate::features::explore::use_toolbar_result_snapshot;
use crate::i18n::{TextKey, t};
use crate::models::SearchCriteria;
use crate::perf;
use crate::state::use_results_context;
use dioxus::prelude::*;
use std::sync::Arc;

// ── private helpers ─────────────────────────────────────────────────────────

// `criteria_snapshot` is used on WASM only (threaded into the download
// there); unused on native, where the parameter exists for signature parity.
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
    mut download_busy: Signal<bool>,
    mut download_status: Signal<Option<String>>,
) {
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
        if let Err(err) = execute_download(
            format,
            #[cfg(target_arch = "wasm32")]
            {
                let Some(criteria_snapshot) = criteria_snapshot else {
                    log::warn!(
                        "event=download phase=table_query state=error reason=missing_criteria_snapshot"
                    );
                    *download_busy.write() = false;
                    *download_status.write() = None;
                    return;
                };
                criteria_snapshot
            },
            query,
            filename,
        )
        .await
        {
            log::warn!(
                "event=download phase=table_fetch state=error format={} reason={err}",
                format.log_name()
            );
        }
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
    download_busy: Signal<bool>,
    download_status: Signal<Option<String>>,
) {
    spawn_query_download(
        spec.format,
        t(locale, spec.status_key).to_string(),
        criteria_snapshot,
        filename,
        query,
        download_busy,
        download_status,
    );
}

fn dispatch_metadata_download_blob(filename: &str, body: &str) {
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
    trigger_download(filename, "application/ld+json", body);
    let elapsed_ms =
        perf::end_timer("LOTUS:table_download_meta_trigger", trigger_timer).as_secs_f64() * 1000.0;
    log::info!(
        "event=download phase=table_trigger state=success format=metadata elapsed_ms={elapsed_ms:.1}"
    );
}

// ── components ─────────────────────────────────────────────────────────────

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
    download_busy: Signal<bool>,
    download_status: Signal<Option<String>>,
    criteria: ReadSignal<SearchCriteria>,
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
                    dispatch_query_download_spec(
                        spec,
                        locale,
                        criteria_snapshot,
                        filename(),
                        sparql_query.clone(),
                        download_busy,
                        download_status,
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
                    dispatch_metadata_download_blob(&filename, metadata_json.as_ref());
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

    // Local download state — busy flag and status text.
    let download_busy = use_signal(|| false);
    let download_status: Signal<Option<String>> = use_signal(|| None);

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
            if *download_busy.read() {
                DownloadStatusSpinner {
                    download_status,
                    locale,
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
                                disabled: *download_busy.read(),
                                download_busy,
                                download_status,
                                criteria,
                                filename: toolbar_model.read().csv_filename.clone(),
                            }
                        }
                        li {
                            DownloadQueryButton {
                                spec: DOWNLOAD_QUERY_JSON_SPEC,
                                sparql_query: query.clone(),
                                locale,
                                disabled: *download_busy.read(),
                                download_busy,
                                download_status,
                                criteria,
                                filename: toolbar_model.read().json_filename.clone(),
                            }
                        }
                        li {
                            DownloadQueryButton {
                                spec: DOWNLOAD_QUERY_RDF_SPEC,
                                sparql_query: query.clone(),
                                locale,
                                disabled: *download_busy.read(),
                                download_busy,
                                download_status,
                                criteria,
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
                                disabled: *download_busy.read(),
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
                                    #[cfg(target_arch = "wasm32")]
                                    {
                                        if let Some(win) = web_sys::window()
                                            && let Some(url) =
                                                ui_url_for_click.as_ref()
                                        {
                                            let _ = win
                                                .open_with_url_and_target(url, "_blank");
                                        }
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
