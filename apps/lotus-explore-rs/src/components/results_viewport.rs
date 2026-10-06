// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Top-level results area component using phase-driven rendering.

use crate::components::copy_button::CopyButton;
use crate::components::loading::{DownloadDispatchState, DownloadOnlyState, LoadingState};
use crate::components::results_table::ResultsTable;
use crate::i18n::{TextKey, t};
use crate::state::use_results_context;
use crate::ui::{ContentPhase, LifecycleBooleans};
use dioxus::prelude::*;
use std::sync::Arc;

#[component]
pub fn ResultsViewport() -> Element {
    use crate::features::explore::ExploreUiState;
    use crate::features::explore::selectors::use_result_selector;

    let state = use_results_context();
    let explore = state.explore;
    let form = crate::state::use_form_criteria_context();
    // The results on screen belong to the criteria as they were when the search
    // ran. Editing the form does not re-run it, so this is what tells the viewport
    // the table no longer describes the query above it.
    let ui_state = use_memo(move || ExploreUiState::from_explore(explore, form.is_dirty()));

    let phase = use_memo(move || {
        let s = *ui_state.read();
        ContentPhase::from(LifecycleBooleans {
            loading: s.loading,
            has_error: s.has_error,
            searched_once: s.searched_once,
            download_only_mode: s.download_only_mode,
            has_entries: s.has_entries,
            criteria_dirty: s.criteria_dirty,
        })
    });

    // The query is available on the error path too, and is offered behind a
    // disclosure rather than shown: a failed search should lead with why it
    // failed, not with the query that produced the failure.
    let sparql_query = use_result_selector(explore, |result| result.sparql_query.clone());
    let searched_once = use_result_selector(explore, |result| result.sparql_query.is_some());

    match *phase.read() {
        ContentPhase::Welcome => rsx! {},
        ContentPhase::Loading => rsx! {
            LoadingState {}
        },
        // Error state: the banner above says what went wrong; this offers the
        // query behind a disclosure for the reader who wants to act on it.
        ContentPhase::Error => {
            let query = sparql_query.read();
            query.as_ref().map_or_else(
                || rsx! {},
                |q| {
                    rsx! { QueryDisplay { query: Arc::clone(q) } }
                },
            )
        }
        // The results are gone because the query they answered is not the query
        // on screen any more. Saying so, and saying what to do, is the difference
        // between a page that looks broken and a page that is waiting for a
        // button press.
        ContentPhase::Stale => rsx! { StaleNotice {} },
        ContentPhase::Empty => {
            if *searched_once.read() {
                rsx! { ResultsTable {} }
            } else {
                rsx! {}
            }
        }
        ContentPhase::Loaded => rsx! {
            ResultsTable {}
        },
        ContentPhase::DownloadOnly => {
            if ui_state.read().download_dispatching {
                rsx! {
                    DownloadDispatchState {}
                }
            } else {
                rsx! {
                    DownloadOnlyState {}
                }
            }
        }
    }
}

/// "Your search criteria changed. Run the search again to see results for them."
#[component]
fn StaleNotice() -> Element {
    let locale = crate::hooks::use_locale();
    rsx! {
        div {
            class: "empty-state",
            role: "status",
            // `polite`: the notice appears when a field is edited and stays
            // there. It is not an error and must not interrupt what the user is
            // still typing.
            aria_live: "polite",
            p {
                class: "text-muted",
                "{t(locale, TextKey::StaleResults)}"
            }
        }
    }
}

/// The query that failed, behind a disclosure rather than in the open.
///
/// This was an always-expanded `<pre>`, and it made the largest element on the
/// page the least useful one: a reader whose search failed was looking at a wall
/// of SPARQL and the reason it failed was in a banner above it. The query is worth
/// having -- it is the only way to see what was actually attempted -- but it is a
/// detail, and it is the reader who asks for it.
///
/// Collapsed by default, matching the SPARQL panel in the results toolbar, and the
/// warning is the summary rather than something beside it: a summary that has to be
/// expanded to find out whether the search worked is the wrong summary.
///
/// Note this is a *reconstruction* of the query, rebuilt from the criteria rather
/// than remembered byte for byte, so a search that resolved a name shows the
/// unresolved shape. That is why it is not labelled as the query that failed: it
/// is close enough to act on and not exact enough to report as fact.
#[component]
fn QueryDisplay(query: std::sync::Arc<str>) -> Element {
    let locale = crate::hooks::use_locale();
    rsx! {
        section {
            id: "query-display",
            class: "query-display w-full max-w-none px-0 pt-3",
            details {
                class: "group overflow-hidden rounded-xl border border-border bg-panel-soft",
                summary {
                    // scroll-mt keeps the row clear of the sticky header when a tap
                    // focuses it; without it the browser scrolls it under the bar.
                    class: "flex w-auto min-w-0 scroll-mt-16 cursor-pointer select-none items-center gap-2 px-3 py-2 text-ui font-semibold text-muted hover:bg-bg focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                    span {
                        class: "inline-block text-subtle transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] group-open:rotate-90",
                        aria_hidden: "true",
                        "▶"
                    }
                    "{t(locale, TextKey::ShowFailedQuery)}"
                }
                div {
                    class: "flex w-full min-w-0 flex-col gap-2 border-t border-border p-3 sm:p-4",
                    p {
                        class: "mx-0 mt-0 mb-0 text-ui leading-relaxed text-muted",
                        "{t(locale, TextKey::FailedQueryReconstructed)}"
                    }
                    CopyButton {
                        text: query.clone(),
                        title: t(locale, TextKey::CopySparqlQuery),
                        locale,
                    }
                    pre {
                        class: "m-0 max-h-96 overflow-y-auto whitespace-pre-wrap break-all rounded-xl border border-border bg-surface p-4 font-mono text-ui text-text",
                        code { class: "block p-0", "{query}" }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "results_viewport/tests.rs"]
mod tests;
