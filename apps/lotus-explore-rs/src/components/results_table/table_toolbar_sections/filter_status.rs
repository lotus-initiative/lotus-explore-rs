// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! How many rows the column filters are currently letting through.
//!
//! A filter that hides rows silently is indistinguishable from a search that found
//! fewer of them, so the count is stated and the way back is offered in the same
//! place. Hidden entirely when nothing is filtered: a permanent counter would be
//! noise on every search.
//!
//! The number arrives as a parameter rather than being counted here, so the figure
//! on screen and the rows on screen cannot disagree.

use crate::features::explore::interactions::use_explore_interactions;
use crate::features::explore::selectors::use_result_selector;
use crate::i18n::{TextKey, format_count, t};
use crate::state::use_results_context;
use dioxus::prelude::*;

#[component]
pub fn FilterStatus(shown: usize) -> Element {
    let locale = crate::hooks::use_locale();
    let state = use_results_context();
    let interactions = use_explore_interactions();
    let total = use_result_selector(state.explore, |result| result.set.row_count());
    let filters = use_result_selector(state.explore, |result| result.filters.clone());

    rsx! {
        if filters.read().is_active() {
            div {
                class: "flex w-full min-w-0 flex-wrap items-center gap-2 text-ui",
                role: "status",
                aria_live: "polite",
                span { class: "tabular-nums text-muted",
                    "{t(locale, TextKey::FilterShowing)} {format_count(locale, shown)} {t(locale, TextKey::FilterOf)} {format_count(locale, *total.read())}"
                }
                button {
                    r#type: "button",
                    class: "cursor-pointer rounded-full border border-border bg-panel px-2.5 py-1 text-ui font-semibold text-muted hover:border-accent hover:text-text focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                    onclick: move |_| {
                        interactions.clear_filters();
                    },
                    "{t(locale, TextKey::ClearFilters)}"
                }
            }
        }
    }
}
