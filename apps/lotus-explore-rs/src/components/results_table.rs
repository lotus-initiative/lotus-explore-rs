// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::features::explore::selectors::{use_result_arc_selector, use_result_selector};
use crate::i18n::{TextKey, t};
use crate::state::use_results_context;
use crate::ui::a11y_contract::{RESULTS_SECTION_HEADING_ID, RESULTS_SECTION_ID};
use dioxus::prelude::*;

mod download_model;
mod header_model;
mod render_model;
mod row_cells;
mod scroll_runtime;
mod sort_helpers;
mod sort_model;
mod table_header;
mod table_toolbar_sections;
mod table_view_model;
mod toolbar;
mod virtualization_controller;
mod virtualized_table;

use table_view_model::{apply_sort, prepare_table_state};
use toolbar::ResultsToolbar;
use virtualized_table::VirtualizedResultsTable;

const TABLE_SCROLL_ID: &str = "results-table-scroll";
const VIRTUAL_OVERSCAN_ROWS: usize = 4;
const ROW_HEIGHT_PX_COMFORTABLE: usize = 138;
const TABLE_VIEWPORT_FALLBACK_PX: usize = 760;

/// Renders the full results section.
#[component]
pub fn ResultsTable() -> Element {
    let state = use_results_context();
    let explore = state.explore;
    let locale = crate::hooks::use_locale();

    // Narrow selectors — each memo fires only for its own slice of state.
    // `entries_arc` uses Arc pointer equality, so sort changes never cause
    // an O(N) deep-equality scan of the result set.
    let entries_arc = use_result_arc_selector(explore, |r| r.entries.clone());
    let sort = use_result_selector(explore, |r| r.sort);

    // Expensive step: row-text derivation + lazy sort-index cache allocation.
    // Depends on `entries_arc` (ptr equality) so it is skipped on sort changes.
    let prepared_state = use_memo(move || prepare_table_state(entries_arc.read().0.clone()));

    // Cheap step: pick the right sort indices without re-running row prep.
    // Fires whenever entries change OR sort changes.
    let table_view_model = use_memo(move || apply_sort(&prepared_state.read(), *sort.read()));

    let total = entries_arc.read().0.len();

    rsx! {
        section {
            id: RESULTS_SECTION_ID,
            "vocab": "https://schema.org/",
            "prefix": "wd: http://www.wikidata.org/entity/ wdt: http://www.wikidata.org/prop/direct/",
            "typeof": "ItemList",
            role: "region",
            aria_label: "{t(locale, TextKey::TableTriplesAria)}",
            aria_labelledby: RESULTS_SECTION_HEADING_ID,
             class: "results-wrap min-h-0 w-full max-w-none px-0",

            "property": "numberOfItems",
            "content": "{total}",
            h2 { id: RESULTS_SECTION_HEADING_ID, class: "sr-only", "{t(locale, TextKey::TableTriplesAria)}" }
            ResultsToolbar {}

            if total == 0 {
                div { class: "w-full mt-5 px-0",
                    div {
                        class: "empty-state",
                        p {
                            class: "text-muted",
                            "{t(locale, TextKey::NoResults)}"
                        }
                    }
                }
             } else {
                 div { class: "results-table-container min-h-0 w-full mt-5 px-0",
                     VirtualizedResultsTable {

                        entries: entries_arc,
                        table_view_model,
                    }
                }
            }
        }
    }
}
