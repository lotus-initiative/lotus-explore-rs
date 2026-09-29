// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Compound identity cell for results-table rows.
//! Renders the compound name link, Wikidata badge, Scholia link, and `InChIKey` badge.

use crate::components::results_table::row_cells::prepared::PreparedRow;
use crate::components::results_table::row_cells::row_text::RowText;
use crate::i18n::{Locale, TextKey, aria_search_inchikey, t};
use dioxus::prelude::*;
use lotus_model::CompoundEntry;

pub(in crate::components::results_table::row_cells) fn compound_cell(
    locale: Locale,
    _text: RowText,
    entry: &CompoundEntry,
    prepared: &PreparedRow,
    _name: &str,
    compound_qid: &str,
) -> Element {
    rsx! {
        td { class: "min-w-0 px-3 py-2.5 align-middle text-ui shadow-[inset_3px_0_0_var(--footer-wd-compound)]",
            div { class: "flex flex-col gap-1 max-w-[30ch]",
                a {
                    "property": "name",
                    "resource": "https://www.wikidata.org/entity/{compound_qid}",
                    href: "https://www.wikidata.org/entity/{compound_qid}",
                    target: "_blank",
                    rel: "noopener noreferrer",
                    class: "block min-h-6 break-words hyphens-auto line-clamp-2 font-semibold leading-snug hover:underline text-wd-compound",
                    "{prepared.display_name}"
                }
                if let Some(smiles) = entry.smiles.as_deref() {
                    span { "property": "wdt:P233", "content": "{smiles}", class: "sr-only" }
                }
            }
            div { class: "mt-1 flex flex-wrap items-center gap-1 max-w-[30ch]",
                a {
                    href: "https://scholia.toolforge.org/chemical/{compound_qid}",
                    target: "_blank",
                    rel: "noopener noreferrer",
                    // WCAG 2.5.3: keep the visible "{qid} • Scholia" text in the accessible
                    // name, then add what the link does.
                    aria_label: "{compound_qid} • Scholia — {t(locale, TextKey::OpenInCompoundScholia)}",
                    class: "inline-flex min-h-7 items-center rounded-full border border-wd-compound/35 bg-surface px-2 py-0.5 text-micro font-semibold uppercase tracking-wide text-wd-compound hover:bg-bg focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28",
                    "{compound_qid} • Scholia"
                }
                if let Some(ik) = entry.inchikey.as_deref() {
                    a {
                        "property": "wdt:P235",
                        "resource": "https://www.wikidata.org/wiki/Special:Search?search={ik}",
                        href: "https://www.wikidata.org/wiki/Special:Search?search={ik}",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        aria_label: "{aria_search_inchikey(locale, ik)}",
                        class: "inline-flex min-h-7 max-w-full break-all items-center rounded-full border border-wd-compound/35 bg-surface px-2 py-0.5 text-micro font-semibold uppercase tracking-wide text-wd-compound hover:bg-bg focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28",
                        "{ik}"
                    }
                }
            }
        }
    }
}
