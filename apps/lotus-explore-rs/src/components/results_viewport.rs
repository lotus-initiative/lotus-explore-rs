// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Top-level results area component using phase-driven rendering.

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

    // Get the SPARQL query to show even on error
    let sparql_query = use_result_selector(explore, |result| result.sparql_query.clone());
    let searched_once = use_result_selector(explore, |result| result.sparql_query.is_some());

    match *phase.read() {
        ContentPhase::Welcome => rsx! {},
        ContentPhase::Loading => rsx! {
            LoadingState {}
        },
        // Error state: show the SPARQL query that was attempted
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

#[component]
fn QueryDisplay(query: std::sync::Arc<str>) -> Element {
    rsx! {
        section {
            id: "query-display",
            class: "w-full max-w-none px-0 pt-3",
            h2 { class: "text-title font-semibold text-text mb-4", "SPARQL Query" }
            pre {
                class: "m-0 max-h-96 overflow-auto font-mono text-ui text-muted whitespace-pre-wrap break-all",
                code { class: "block p-0", "{query}" }
            }
        }
    }
}
