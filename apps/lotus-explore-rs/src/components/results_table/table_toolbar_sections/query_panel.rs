// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::components::copy_button::CopyButton;
use crate::features::explore::use_toolbar_result_snapshot;
use crate::i18n::{TextKey, t};
use crate::state::use_results_context;
use dioxus::prelude::*;

#[component]
pub fn QueryPanel() -> Element {
    let locale = crate::hooks::use_locale();
    let explore = use_results_context().explore;
    let toolbar_snapshot = use_toolbar_result_snapshot(explore);

    rsx! {
        if let Some(q) = toolbar_snapshot.read().sparql_query.as_ref() {
            section {
                class: "query-panel w-auto max-w-none px-0 mt-2",
                aria_label: "{t(locale, TextKey::SparqlQuery)}",
                details {
                    class: "group rounded-xl border border-border bg-panel-soft",
                    summary {
                        // scroll-mt keeps the row clear of the sticky header when a
                        // tap focuses it; without it the browser scrolls it under the bar.
                        class: "flex w-auto min-w-0 min-h-[44px] scroll-mt-16 cursor-pointer select-none items-center gap-2 rounded-xl group-open:rounded-b-none px-3 py-2 text-ui font-semibold text-muted hover:bg-bg focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2",
                        span {
                            class: "inline-block text-subtle transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] group-open:rotate-90",
                            aria_hidden: "true",
                            "▶"
                        },
                        "{t(locale, TextKey::SparqlQuery)}"
                    }
                    div { class: "flex w-full min-w-0 flex-col gap-2 border-t border-border p-3 sm:p-4",
                        CopyButton {
                            text: q.clone(),
                            title: t(locale, TextKey::CopySparqlQuery),
                            locale,
                        }
                        pre {
                            tabindex: "0",
                            aria_label: "{t(locale, TextKey::SparqlQuery)}",
                            class: "m-0 max-h-96 overflow-auto whitespace-pre rounded-xl border border-border bg-surface p-4 font-mono text-ui text-text focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2",
                            "{q.as_ref()}"
                        }
                    }
                }
            }
        }
    }
}
