// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Virtualized results table body and WASM scroll scheduling glue.

use super::render_model::build_virtualized_table_render_model;
use super::row_cells::{ResultsRowsWindow, row_text};
use super::table_filter_row::TableFilterRow;
use super::table_header::TableHeader;
use super::table_view_model::TableViewModel;
use super::virtualization_controller::use_results_table_virtualization;
use crate::features::explore::interactions::use_explore_interactions;
use crate::i18n::{TextKey, t};
use dioxus::prelude::*;

#[component]
pub(super) fn VirtualizedResultsTable(table_view_model: Memo<TableViewModel>) -> Element {
    let locale = crate::hooks::use_locale();
    let interactions = use_explore_interactions();
    // Virtualization is driven by the rows on screen, which is the sorted order
    // minus whatever the column filters exclude — not by the rows the search
    // returned. Spacing off the unfiltered count would leave blank bands at the
    // bottom of the scroll area whenever a filter was narrowing.
    let view_model = table_view_model.read();
    let total = view_model.visible_row_count();
    let virtualization = use_results_table_virtualization(total);
    let text = row_text(locale);

    let render_model = build_virtualized_table_render_model(&view_model, virtualization.state);
    let mut effect_virtualization = virtualization.clone();
    let scroll_virtualization = virtualization.clone();

    use_effect(move || {
        effect_virtualization.sync_after_render(total);
    });

    let on_scroll = move |_| scroll_virtualization.handle_scroll(total);

    rsx! {
        div {
            id: virtualization.config.scroll_id,
            // Focusable so the scroll area is reachable and arrow-scrollable from the
            // keyboard. Deliberately not `role="region"`: the results `<section>`
            // outside is already a region named "Compound-taxon-reference triples", and
            // a second region with the same name is worse than none — a screen reader
            // listing landmarks showed two identically-named entries and could not say
            // which was the table. axe calls this `landmark-unique`.
            //
            // `aria-labelledby` instead of a second `aria-label` for the same reason:
            // one name, from the caption that already carries it.
            tabindex: "0",
             class: "results-table-scroll scrollbar-hidden-x w-full max-w-none rounded-xl border border-shell-border bg-shell-raised",
             onscroll: on_scroll,

            table {
                // No `aria-label`: the caption below names the table, and an
                // `aria-label` would override it rather than agree with it.
                // A floor, not a width. The `colgroup` below states what each
                // column asks for; this only stops `table-auto` crushing them
                // on a narrow viewport. It sits about 50px above the widest
                // breakpoint's sum, which is the slack the table had before the
                // filter row needed room in the mass and year columns.
                class: "w-full min-w-[1500px] table-auto border-collapse text-ui",
                caption { class: "sr-only", "{t(locale, TextKey::TableTriplesAria)}" }
                colgroup {
                    col { class: "w-[50px] sm:w-[50px] lg:w-[50px] min-w-[50px] max-w-[50px]" }
                    col { class: "w-[26ch] sm:w-[28ch] lg:w-[30ch]" }
                    col { class: "w-[16ch] sm:w-[16ch] lg:w-[16ch]" }
                    col { class: "w-[12ch] sm:w-[12ch] lg:w-[12ch]" }
                    col { class: "w-[22ch] sm:w-[24ch] lg:w-[26ch]" }
                    col { class: "w-[24ch] sm:w-[26ch] lg:w-[28ch]" }
                    col { class: "w-[13ch] sm:w-[13ch] lg:w-[13ch]" }
                }
                thead {
                    class: "sticky top-0 z-2",
                    TableHeader {
                        current_sort: render_model.current_sort,
                        on_sort_toggle: move |col| interactions.toggle_sort(col),
                    }
                    TableFilterRow {}
                }
                tbody {
                    if render_model.has_top_spacer() {
                        tr { aria_hidden: "true",
                            td {
                                colspan: "7",
                                class: "virtual-spacer",
                                style: "height: {render_model.top_spacer_px}px",
                            }
                        }
                    }
                    // Filters that exclude everything have to say so. An empty
                    // table under a filter row reads as a broken table, and the
                    // filter row itself is what is wrong, so the way out has to be
                    // on screen rather than up in the toolbar.
                    if total == 0 {
                        tr {
                            td {
                                colspan: "7",
                                class: "px-3 py-6 text-center text-ui text-muted",
                                "{t(locale, TextKey::FilterNoMatches)}"
                            }
                        }
                    }
                    ResultsRowsWindow {
                        locale,
                        text,
                        rows: render_model.rows.clone(),
                        prepared_rows: render_model.prepared_rows.clone(),
                        keys: render_model.keys.clone(),
                    }
                    if render_model.has_bottom_spacer() {
                        tr { aria_hidden: "true",
                            td {
                                colspan: "7",
                                class: "virtual-spacer",
                                style: "height: {render_model.bottom_spacer_px}px",
                            }
                        }
                    }
                }
            }
        }
    }
}
