// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::curation::{CurationResultRow, CurationStatus};
use crate::i18n::{
    Locale, TextKey, col_canonical_smiles, col_exact_mass, col_name, col_original_smiles,
    col_status, curation_badge_mass_missing, curation_badge_prerequisite_pending,
    curation_badge_second_pass_required, curation_mass_warning_title, curation_status_label,
    hint_scroll_curation_results, label_new_item, t,
};
use dioxus::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

const NA_TEXT: &str = "n/a";

fn status_badge_class(status: &CurationStatus) -> &'static str {
    match status {
        CurationStatus::ExistingComplete => {
            "inline-flex items-center rounded-full border border-shell-border bg-shell-raised px-2 py-0.5 text-micro font-semibold uppercase tracking-wide text-wd-taxon"
        }
        CurationStatus::ExistingNeedsUpdates => {
            "inline-flex items-center rounded-full border border-shell-border bg-shell-raised px-2 py-0.5 text-micro font-semibold uppercase tracking-wide text-wd-entries"
        }
        CurationStatus::NewCompound | CurationStatus::PendingDependencies => {
            "inline-flex items-center rounded-full border border-shell-border bg-shell-raised px-2 py-0.5 text-micro font-semibold uppercase tracking-wide text-wd-reference"
        }
        CurationStatus::Error => {
            "inline-flex items-center rounded-full border border-shell-border bg-shell-raised px-2 py-0.5 text-micro font-semibold uppercase tracking-wide text-wd-compound"
        }
    }
}

#[component]
fn StatusSummaryBadges(locale: Locale, rows: Arc<[CurationResultRow]>) -> Element {
    rsx! {
        div { class: "flex flex-wrap gap-1.5",
            for (status, count) in status_counts(rows.as_ref()) {
                span {
                    class: status_badge_class(&status),
                    "{status_label(locale, &status)} ({count})"
                }
            }
        }
    }
}

fn render_curation_result_cells(locale: Locale, row: &CurationResultRow) -> Element {
    rsx! {
        td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text",
            span {
                class: status_badge_class(&row.status),
                "{status_label(locale, &row.status)}"
            }
            div { class: "mt-1 flex flex-wrap gap-1",
                if !row.dependency_blocks.is_empty() {
                    span { class: "inline-flex items-center rounded-full border border-shell-border bg-shell-raised px-2 py-0.5 text-micro font-semibold uppercase tracking-wide text-wd-reference",
                        "{curation_badge_prerequisite_pending(locale)}"
                    }
                }
                if matches!(row.status, CurationStatus::PendingDependencies) {
                    span { class: "inline-flex items-center rounded-full border border-shell-border bg-shell-raised px-2 py-0.5 text-micro font-semibold uppercase tracking-wide text-wd-reference",
                        "{curation_badge_second_pass_required(locale)}"
                    }
                }
                if row.exact_mass.is_none() {
                    span {
                        class: "inline-flex items-center rounded-full border border-shell-border bg-shell-raised px-2 py-0.5 text-micro font-semibold uppercase tracking-wide text-wd-entries",
                        title: "{row.mass_warning.as_deref().unwrap_or(curation_mass_warning_title(locale))}",
                        "{curation_badge_mass_missing(locale)}"
                    }
                }
            }
            if !row.note.is_empty() {
                div { class: "mt-1 whitespace-pre-line text-micro text-muted", "{row.note}" }
            }
        }
        td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text",
            if let Some(qid) = row.wikidata_qid.as_deref() {
                a {
                    class: "font-mono text-micro text-wd-compound underline break-all",
                    href: "https://www.wikidata.org/wiki/{qid}",
                    target: "_blank",
                    rel: "noopener noreferrer",
                    "{qid}"
                }
            } else {
                span { class: "text-muted", "{label_new_item(locale)}" }
            }
        }
        td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text", "{row.input.name}" }
        td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text font-mono text-micro break-all", "{row.input.smiles}" }
        td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text font-mono text-micro break-all", "{row.canonical_smiles.as_deref().unwrap_or(NA_TEXT)}" }
        td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text font-mono text-micro break-all", "{row.inchikey.as_deref().unwrap_or(NA_TEXT)}" }
        td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text font-mono text-micro break-all", "{row.inchi.as_deref().unwrap_or(NA_TEXT)}" }
        td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text font-mono text-micro break-all", "{row.formula.as_deref().unwrap_or(NA_TEXT)}" }
        td { class: "border-b border-panel-border px-3 py-2.5 align-top text-ui text-text font-mono text-micro break-all", "{format_mass(row.exact_mass)}" }
    }
}

#[component]
pub fn CurationResultsTable(locale: Locale, rows: Arc<[CurationResultRow]>) -> Element {
    let scroll_hint_id = "curation-results-scroll-hint";

    rsx! {
        div { class: "flex flex-col gap-3",
            h3 { class: "text-base font-semibold", "{crate::i18n::heading_results(locale)}" }
            StatusSummaryBadges { locale, rows: rows.clone() }
            p {
                id: scroll_hint_id,
                class: "inline-flex items-center gap-2 text-micro text-muted",
                span { class: "text-sm font-bold text-accent", "↔" }
                "{hint_scroll_curation_results(locale)}"
            }
            div {
                class: "w-full overflow-x-auto rounded-xl border border-shell-border bg-shell-raised focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/40",
                role: "region",
                tabindex: "0",
                aria_label: "{crate::i18n::heading_results(locale)}",
                aria_describedby: scroll_hint_id,
                table { class: "curation-results-table w-full min-w-[1320px] table-auto border-collapse text-ui",
                    thead {
                        tr { class: "text-left",
                            th { scope: "col", class: "border-b border-shell-border bg-shell-raised px-3 py-2 text-left text-micro font-semibold uppercase tracking-wide text-muted w-[160px] min-w-[160px]", "{col_status(locale)}" }
                            th { scope: "col", class: "border-b border-shell-border bg-shell-raised px-3 py-2 text-left text-micro font-semibold uppercase tracking-wide text-muted w-[8ch] min-w-[8ch]", "Wikidata" }
                            th { scope: "col", class: "border-b border-shell-border bg-shell-raised px-3 py-2 text-left text-micro font-semibold uppercase tracking-wide text-muted w-[14ch] min-w-[14ch]", "{col_name(locale)}" }
                            th { scope: "col", class: "border-b border-shell-border bg-shell-raised px-3 py-2 text-left text-micro font-semibold uppercase tracking-wide text-muted w-[180px] min-w-[180px]", "{col_original_smiles(locale)}" }
                            th { scope: "col", class: "border-b border-shell-border bg-shell-raised px-3 py-2 text-left text-micro font-semibold uppercase tracking-wide text-muted w-[180px] min-w-[180px]", "{col_canonical_smiles(locale)}" }
                            th { scope: "col", class: "border-b border-shell-border bg-shell-raised px-3 py-2 text-left text-micro font-semibold uppercase tracking-wide text-muted w-[180px] min-w-[180px]", "InChIKey" }
                            th { scope: "col", class: "border-b border-shell-border bg-shell-raised px-3 py-2 text-left text-micro font-semibold uppercase tracking-wide text-muted w-[180px] min-w-[180px]", "InChI" }
                            th { scope: "col", class: "border-b border-shell-border bg-shell-raised px-3 py-2 text-left text-micro font-semibold uppercase tracking-wide text-muted w-[8ch] min-w-[8ch]", "{t(locale, TextKey::Formula)}" }
                            th { scope: "col", class: "border-b border-shell-border bg-shell-raised px-3 py-2 text-left text-micro font-semibold uppercase tracking-wide text-muted w-[8ch] min-w-[8ch]", "{col_exact_mass(locale)}" }
                        }
                    }
                    tbody {
                        for (idx, row) in rows.iter().enumerate() {
                            tr { key: "{row.inchikey.as_deref().unwrap_or(&idx.to_string())}",
                                class: "odd:bg-surface/30 hover:bg-surface/40",
                                {render_curation_result_cells(locale, row)}
                            }
                        }
                    }
                }
            }
        }
    }
}

fn status_label(locale: Locale, status: &CurationStatus) -> &'static str {
    let key = match status {
        CurationStatus::ExistingComplete => "existing_complete",
        CurationStatus::ExistingNeedsUpdates => "existing_updates",
        CurationStatus::NewCompound => "new_compound",
        CurationStatus::PendingDependencies => "pending_dependencies",
        CurationStatus::Error => "error",
    };
    curation_status_label(locale, key)
}

fn status_counts(rows: &[CurationResultRow]) -> Vec<(CurationStatus, usize)> {
    let mut counts = HashMap::<CurationStatus, usize>::new();
    for row in rows {
        *counts.entry(row.status.clone()).or_insert(0) += 1;
    }

    let ordered = [
        CurationStatus::ExistingComplete,
        CurationStatus::ExistingNeedsUpdates,
        CurationStatus::NewCompound,
        CurationStatus::PendingDependencies,
        CurationStatus::Error,
    ];

    ordered
        .into_iter()
        .filter_map(|status| counts.get(&status).copied().map(|count| (status, count)))
        .collect::<Vec<_>>()
}

fn format_mass(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".to_string(), |m| format!("{m:.5}"))
}
