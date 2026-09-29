// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Reference identity cell for results-table rows.

use crate::components::results_table::row_cells::prepared::PreparedRow;
use crate::components::results_table::row_cells::row_text::RowText;
use crate::i18n::{Locale, TextKey, aria_wikidata_statement, t};
use crate::models::CompoundEntry;
use dioxus::prelude::*;

pub(in crate::components::results_table::row_cells) fn reference_cell(
    locale: Locale,
    text: RowText,
    entry: &CompoundEntry,
    prepared: &PreparedRow,
    reference_qid: &str,
) -> Element {
    let doi = prepared.doi.as_deref();
    let statement_id = prepared.statement_id.as_deref();
    rsx! {
        td { "property": "wdt:P248", "typeof": "ScholarlyArticle", "resource": "https://www.wikidata.org/entity/{reference_qid}", class: "min-w-0 px-3 py-2.5 align-middle text-ui shadow-[inset_3px_0_0_var(--footer-wd-reference)]",
            div { class: "flex flex-col gap-1",
                if let Some(full_title) = entry.ref_title.as_deref() {
                    a {
                        href: "https://www.wikidata.org/entity/{reference_qid}",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        class: "block min-h-6 break-words line-clamp-2 font-semibold leading-snug hover:underline text-wd-reference",
                        "{full_title}"
                    }
                } else {
                    a {
                        href: "https://www.wikidata.org/entity/{reference_qid}",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        class: "block min-h-6 break-words line-clamp-2 font-semibold leading-snug hover:underline text-wd-reference",
                        "{reference_qid}"
                    }
                }
            }
            div { class: "mt-1 flex flex-wrap items-center gap-1",
                a {
                    href: "https://scholia.toolforge.org/work/{reference_qid}",
                    target: "_blank",
                    rel: "noopener noreferrer",
                    // WCAG 2.5.3: keep the visible "{qid} • Scholia" text in the accessible
                    // name, then add what the link does.
                    aria_label: "{reference_qid} • Scholia — {t(locale, TextKey::OpenInReferenceScholia)}",
                    class: "inline-flex min-h-7 items-center rounded-full border border-wd-reference/35 bg-surface px-2 py-0.5 text-micro font-semibold uppercase tracking-wide text-wd-reference hover:bg-bg focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28",
                    "{reference_qid} • Scholia"
                }
                if let Some(d) = doi {
                    a {
                        "property": "identifier",
                        "resource": "https://doi.org/{d}",
                        href: "https://doi.org/{d}",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        class: "inline-flex min-h-7 items-center rounded-full border border-wd-reference/35 bg-surface px-2 py-0.5 text-micro font-semibold uppercase tracking-wide text-wd-reference hover:bg-bg focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28",
                        // WCAG 2.5.3: the visible DOI text stays in the
                        // accessible name, then what the link does.
                        aria_label: "{d} — {text.open_doi}",
                        "{d}"
                    }
                }
                if let Some(stmt) = statement_id {
                    a {
                        href: "https://www.wikidata.org/entity/statement/{stmt}",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        class: "inline-flex min-h-7 items-center rounded-full border border-wd-reference/35 bg-surface px-2 py-0.5 text-micro font-semibold uppercase tracking-wide text-wd-reference hover:bg-bg focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28",
                        // Same rule as the DOI badge above.
                        aria_label: "{stmt} — {aria_wikidata_statement(locale, stmt)}",
                        "{stmt}"
                    }
                }
            }
        }
    }
}
