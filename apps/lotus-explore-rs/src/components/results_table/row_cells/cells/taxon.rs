// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Taxon identity cell for results-table rows.
//! Renders the taxon name link and Scholia badge.

use crate::components::results_table::row_cells::row_text::RowText;
use crate::i18n::{Locale, TextKey, t};
use dioxus::prelude::*;
use lotus_model::CompoundEntry;

pub(in crate::components::results_table::row_cells) fn taxon_cell(
    locale: Locale,
    _text: RowText,
    entry: &CompoundEntry,
    taxon_qid: &str,
) -> Element {
    // A row can arrive with no taxon: a structure search returns compounds whether
    // or not their occurrence data exists, and a compound with no `P703` has nothing
    // to file it under. Rendering the badge anyway produced " • Scholia" — a dangling
    // bullet wrapping a link to `scholia.toolforge.org/taxon/`, a search page for the
    // empty string.
    if taxon_qid.trim().is_empty() {
        return rsx! {
            td {
                "property": "wdt:P703",
                "typeof": "Taxon",
                class: "min-w-0 px-3 py-2.5 align-middle text-ui shadow-[inset_3px_0_0_var(--footer-wd-taxon)]",
            }
        };
    }

    let display_name: &str = if entry.taxon_name.trim().is_empty() {
        taxon_qid
    } else {
        &entry.taxon_name
    };

    rsx! {
        td { "property": "wdt:P703", "typeof": "Taxon", "resource": "https://www.wikidata.org/entity/{taxon_qid}", class: "min-w-0 px-3 py-2.5 align-middle text-ui shadow-[inset_3px_0_0_var(--footer-wd-taxon)]",
            div { class: "flex flex-col gap-1 max-w-[24ch]",
                a {
                    "property": "name scientificName",
                    href: "https://www.wikidata.org/entity/{taxon_qid}",
                    target: "_blank",
                    rel: "noopener noreferrer",
                    class: "inline-flex min-h-[44px] items-center py-1 break-words font-semibold italic leading-snug hover:underline text-wd-taxon",
                    span { class: "line-clamp-2", "{display_name}" }
                }
            }
            div { class: "mt-1 flex flex-wrap items-center gap-2 max-w-[24ch]",
                a {
                    href: "https://scholia.toolforge.org/taxon/{taxon_qid}",
                    target: "_blank",
                    rel: "noopener noreferrer",
                    // WCAG 2.5.3: keep the visible "{qid} • Scholia" text in the accessible
                    // name, then add what the link does.
                    aria_label: "{taxon_qid} • Scholia — {t(locale, TextKey::OpenInTaxonScholia)}",
                    class: "inline-flex min-h-[44px] min-w-[44px] items-center rounded-full border border-wd-taxon/35 bg-surface px-3 py-2 text-micro font-semibold uppercase tracking-wide text-wd-taxon hover:bg-bg focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2",
                    "{taxon_qid} • Scholia"
                }
            }
        }
    }
}
