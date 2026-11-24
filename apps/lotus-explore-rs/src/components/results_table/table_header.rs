// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Sortable column header for the results table.

use super::header_model::{SortableHeaderModel, build_sortable_header_models};
use crate::i18n::{TextKey, aria_sort_toggle, t};
use dioxus::prelude::*;
use lotus_explore_core::sort::{SortColumn, SortState};

#[component]
pub fn TableHeader(current_sort: SortState, on_sort_toggle: EventHandler<SortColumn>) -> Element {
    let locale = crate::hooks::use_locale::use_locale();
    let headers = build_sortable_header_models(current_sort);

    rsx! {
        tr {
            class: "border-b border-shell-border bg-shell-raised text-left text-muted",
            th {
                scope: "col",
                class: "px-3 sm:px-4 py-2.5 text-ui font-bold whitespace-nowrap select-none",
                span { class: "inline-flex min-h-[44px] items-center", "{t(locale, TextKey::Structure)}" }
            }
            for header in headers {
                SortableColumnHeader {
                    header,
                    on_toggle: on_sort_toggle,
                }
            }
        }
    }
}

#[component]
fn SortableColumnHeader(
    header: SortableHeaderModel,
    on_toggle: EventHandler<SortColumn>,
) -> Element {
    let locale = crate::hooks::use_locale::use_locale();
    let label_text = t(locale, header.label);
    let sort_aria = aria_sort_toggle(locale, label_text, header.next_descending);
    rsx! {
        th {
            scope: "col",
            aria_sort: "{header.aria_sort}",
            class: "px-3 sm:px-4 py-1 text-ui font-bold whitespace-nowrap select-none",
            button {
                r#type: "button",
                aria_label: "{sort_aria}",
                title: "{sort_aria}",
                class: "inline-flex min-h-[44px] w-full min-w-0 items-center gap-1.5 rounded-lg border-0 bg-transparent px-0 py-2 text-inherit hover:text-accent focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2 active:scale-[0.98]",
                onclick: move |_| on_toggle.call(header.col),
                span { class: "block min-w-0 whitespace-nowrap leading-tight", "{label_text}" }
                span {
                    class: "text-ui font-bold leading-none text-subtle",
                    "aria-hidden": "true",
                    {header.sort_icon}
                }
            }
        }
    }
}
