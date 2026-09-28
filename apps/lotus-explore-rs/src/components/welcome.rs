// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Search examples shown below the search form.

use crate::components::copy_button::CopyButton;
use crate::components::ui::Card;
use crate::features::explore::absolute_current_url_with_query;
use crate::i18n::{TextKey, t};
use dioxus::prelude::*;
use std::sync::Arc;

#[component]
pub fn SearchExamples() -> Element {
    let locale = crate::hooks::use_locale();

    rsx! {
        section {
            "vocab": "https://schema.org/",
            "prefix": "wd: http://www.wikidata.org/entity/ wdt: http://www.wikidata.org/prop/direct/",
            class: "w-full",
            Card {
                        class: "flex flex-col gap-3 pb-4 sm:pb-6",
                        h2 { class: "text-body font-semibold text-text", "{t(locale, TextKey::SearchExamples)}" }
                        details {
                            class: "group overflow-hidden",
                            summary {
                                class: "flex w-full min-w-0 cursor-pointer select-none items-center gap-2 rounded-xl bg-panel-soft px-3 py-2 text-ui font-semibold text-muted hover:bg-bg focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                                span {
                                    class: "inline-block text-subtle transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] group-open:rotate-90",
                                    aria_hidden: "true",
                                    "▶"
                                },
                                "{t(locale, TextKey::ExampleApiUrls)}"
                            }
                            div { class: "flex w-full min-w-0 flex-col gap-2 bg-panel-soft p-3 sm:p-4",
                                div {
                                    class: "mt-1 grid grid-cols-1 gap-2.5 md:grid-cols-2",
                                    DownloadExampleRow {
                                        locale,
                                        format: t(locale, TextKey::ExampleQueryExecute),
                                        query: "?taxon=Gentiana%20lutea&execute=true",
                                        id: "example-query-execute",
                                        name: "example-query-execute",
                                    }

                                    DownloadExampleRow {
                                        locale,
                                        format: t(locale, TextKey::ExampleQueryTaxon),
                                        query: "?taxon=*&download=true&format=csv",
                                        id: "example-query-taxon",
                                        name: "example-query-taxon",
                                    }

                                    DownloadExampleRow {
                                        locale,
                                        format: t(locale, TextKey::ExampleQueryStructure),
                                        query: "?structure=c1ccccc1&structure_search_type=similarity&smiles_threshold=0.85&download=true&format=json",
                                        id: "example-query-structure",
                                        name: "example-query-structure",
                                    }

                                    DownloadExampleRow {
                                        locale,
                                        format: t(locale, TextKey::ExampleQueryAdvanced),
                                        query: "?taxon=Fungi&mass_filter=true&mass_min=0&mass_max=300&year_filter=true&year_start=2000&year_end=2026&formula_filter=true&c_min=1&c_max=10&cl_state=required&br_state=excluded&download=true&format=rdf",
                                        id: "example-query-advanced",
                                        name: "example-query-advanced",
                                    }

                                }
                            }
                        }
                    }
                }
    }
}

#[component]
fn DownloadExampleRow(
    locale: crate::i18n::Locale,
    format: &'static str,
    query: &'static str,
    id: &'static str,
    name: &'static str,
) -> Element {
    let absolute = absolute_current_url_with_query(query.trim_start_matches('?'));
    let absolute = Arc::<str>::from(absolute);
    rsx! {
        div {
            class: "flex items-center gap-2 rounded-xl border border-shell-border bg-shell-page p-2 text-ui",
            span {
                class: "shrink-0 rounded-full bg-accent/12 px-2 py-0.5 text-micro font-semibold text-accent",
                "{format}"
            }
            input {
                id: "{id}",
                name: "{name}",
                r#type: "text",
                autocomplete: "off",
                readonly: true,
                value: "{absolute}",
                aria_label: "{format}",
                class: "min-w-0 flex-1 truncate rounded-xl border border-border bg-surface px-2.5 py-1 font-mono text-ui text-muted shadow-xs focus:outline-none focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
            }
            CopyButton { text: absolute.clone(), locale }
        }
    }
}
