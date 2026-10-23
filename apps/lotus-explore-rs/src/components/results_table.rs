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
mod scroll_signals;
mod sort_helpers;
mod sort_model;
mod table_filter_row;
mod table_header;
mod table_toolbar_sections;
mod table_view_model;
mod toolbar;
mod virtualization_controller;
mod virtualized_table;

use table_toolbar_sections::FilterStatus;
use table_view_model::{apply_sort_and_filters, prepare_table_state};
use toolbar::ResultsToolbar;
use virtualized_table::VirtualizedResultsTable;

const TABLE_SCROLL_ID: &str = "results-table-scroll";
const VIRTUAL_OVERSCAN_ROWS: usize = 4;
const ROW_HEIGHT_PX_COMFORTABLE: usize = 138;
const TABLE_VIEWPORT_FALLBACK_PX: usize = 760;

/// A deterministic Wikidata QID for a test's label.
///
/// The store only keeps a cell it recognises as a QID -- `Q-Alpha` is not one,
/// and a row keyed by nothing is dropped rather than shown with a broken link --
/// so a fixture has to use numeric ids. Derived from the label so a test states
/// one thing per row instead of inventing an id to go with it.
///
/// Lives at the parent module because both `sort_model`'s and
/// `table_view_model`'s tests spell it, and did spell it twice.
#[cfg(test)]
fn qid_for(prefix: &str, label: &str) -> std::sync::Arc<str> {
    let mut n: u32 = 2_166_136_261;
    for byte in label.bytes() {
        n = n.wrapping_mul(16_777_619).wrapping_add(u32::from(byte));
    }
    std::sync::Arc::<str>::from(format!("{prefix}{}", n % 1_000_000 + 1))
}

/// Renders the full results section.
#[component]
pub fn ResultsTable() -> Element {
    let state = use_results_context();
    let explore = state.explore;
    let locale = crate::hooks::use_locale();

    // Narrow selectors — each memo fires only for its own slice of state.
    // `entries_arc` uses Arc pointer equality, so sort changes never cause
    // an O(N) deep-equality scan of the result set.
    let set_arc = use_result_arc_selector(explore, |r| r.set.clone());
    let sort = use_result_selector(explore, |r| r.sort);
    let filters = use_result_selector(explore, |r| r.filters.clone());

    // Expensive step: the lazy sort-index cache. Depends on `set_arc` (pointer
    // equality) so it is skipped on sort and filter changes.
    let prepared_state = use_memo(move || prepare_table_state(set_arc.read().0.clone()));

    // Cheap step: pick the right sort indices, then drop what the column
    // filters exclude. Fires whenever entries, sort or filters change.
    let table_view_model = use_memo(move || {
        apply_sort_and_filters(&prepared_state.read(), *sort.read(), &filters.read())
    });

    // What the search returned, as opposed to what the filters let through. The
    // two are reported separately so a filter cannot look like a smaller result.
    let total = set_arc.read().0.row_count();
    let shown = table_view_model.read().visible_row_count();

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
                 FilterStatus { shown }
                 div { class: "results-table-container min-h-0 w-full mt-3 px-0",
                     VirtualizedResultsTable { table_view_model }
                 }
             }
        }
    }
}
