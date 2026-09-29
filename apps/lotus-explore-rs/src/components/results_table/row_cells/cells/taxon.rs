// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Taxon identity cell for results-table rows.
//! Renders the taxon name link and Scholia/Wikidata badges.

use crate::components::results_table::row_cells::row_text::RowText;
use crate::i18n::{Locale, TextKey, t};
use dioxus::prelude::*;
use lotus_model::CompoundEntry;

pub(in crate::components::results_table::row_cells) fn taxon_cell(
    _locale: Locale,
    _text: RowText,
    entry: &CompoundEntry,
    taxon_qid: &str,
) -> Element {
    rsx! {
        td { "property": "wdt:P171", "typeof": "Taxon", "resource": "https://www.wikidata.org/entity/{taxon_qid}", class: "min-w-0 px-3 py-2.5 align-middle text-ui shadow-[inset_3px_0_0_var(--footer-wd-taxon)]",
            div { class: "flex flex-col gap-1 max-w-[24ch]",
                a {
                    href: "https://www.wikidata.org/entity/{taxon_qid}",
                    target: "_blank",
                    rel: "noopener noreferrer",
                    class: "block min-h-6 break-words line-clamp-2 font-semibold italic leading-snug hover:underline text-wd-taxon",
                    "{entry.taxon_name}"
                }
            }
            div { class: "mt-1 flex flex-wrap items-center gap-1 max-w-[24ch]",
                a {
                    href: "https://scholia.toolforge.org/taxon/{taxon_qid}",
                    target: "_blank",
                    rel: "noopener noreferrer",
                    // WCAG 2.5.3: keep the visible "{qid} • Scholia" text in the accessible
                    // name, then add what the link does.
                    aria_label: "{taxon_qid} • Scholia — {t(_locale, TextKey::OpenInTaxonScholia)}",
                    class: "inline-flex min-h-7 items-center rounded-full border border-wd-taxon/35 bg-surface px-2 py-0.5 text-micro font-semibold uppercase tracking-wide text-wd-taxon hover:bg-bg focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28",
                    "{taxon_qid} • Scholia"
                }
            }
        }
    }
}
