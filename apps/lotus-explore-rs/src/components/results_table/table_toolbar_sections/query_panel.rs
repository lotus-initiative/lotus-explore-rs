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
                    class: "group overflow-hidden",
                    summary {
                        class: "flex w-auto min-w-0 cursor-pointer select-none items-center gap-2 rounded-xl border border-border bg-panel-soft px-3 py-2 text-ui font-semibold text-muted hover:bg-bg focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                        span {
                            class: "inline-block text-subtle transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] group-open:rotate-90",
                            aria_hidden: "true",
                            "▶"
                        },
                        "{t(locale, TextKey::SparqlQuery)}"
                    }
                    div { class: "flex w-full min-w-0 flex-col gap-2 bg-panel-soft p-3 sm:p-4",
                        CopyButton {
                            text: q.clone(),
                            title: t(locale, TextKey::CopySparqlQuery),
                            locale,
                        }
                        pre {
                            class: "m-0 max-h-96 overflow-y-auto whitespace-pre-wrap break-all rounded-xl border border-border bg-surface p-4 font-mono text-ui text-text",
                            "{q.as_ref()}"
                        }
                    }
                }
            }
        }
    }
}
