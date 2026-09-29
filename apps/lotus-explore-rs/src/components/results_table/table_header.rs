// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Sortable column header for the results table.

use super::header_model::{SortableHeaderModel, build_sortable_header_models};
use crate::i18n::{TextKey, aria_sort_toggle, t};
use crate::sort::{SortColumn, SortState};
use dioxus::prelude::*;

#[component]
pub fn TableHeader(current_sort: SortState, on_sort_toggle: EventHandler<SortColumn>) -> Element {
    let locale = crate::hooks::use_locale();
    let headers = build_sortable_header_models(current_sort);

    rsx! {
        tr {
            class: "border-b border-shell-border bg-shell-raised text-left text-text2",
            th {
                scope: "col",
                class: "px-3 sm:px-4 py-3 text-ui font-bold whitespace-nowrap select-none",
                span { "{t(locale, TextKey::Structure)}" }
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
    let locale = crate::hooks::use_locale();
    let label_text = t(locale, header.label);
    let sort_aria = aria_sort_toggle(locale, label_text, header.next_descending);
    rsx! {
        th {
            scope: "col",
            aria_sort: "{header.aria_sort}",
            class: "px-3 sm:px-4 py-3 text-ui font-bold whitespace-nowrap select-none",
            button {
                r#type: "button",
                aria_label: "{sort_aria}",
                title: "{sort_aria}",
                class: "inline-flex w-full min-w-0 items-center gap-1.5 border-0 bg-transparent p-0 text-inherit hover:text-accent focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 active:scale-[0.98]",
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
