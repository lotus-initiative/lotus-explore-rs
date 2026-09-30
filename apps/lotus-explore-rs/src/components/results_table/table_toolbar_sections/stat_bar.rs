// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::features::explore::use_toolbar_result_snapshot;
use crate::i18n::{CountNoun, TextKey, count_label, format_count, t};
use crate::state::use_results_context;
use dioxus::prelude::*;
use lotus_model::DatasetStats;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatStripe {
    Compound,
    Taxon,
    Reference,
    Entries,
}

#[component]
fn StatBadge(
    value: usize,
    secondary_value: Option<usize>,
    secondary_label: Option<&'static str>,
    noun: CountNoun,
    plus: bool,
    stripe: StatStripe,
) -> Element {
    let locale = crate::hooks::use_locale();
    let mut display_value = format_count(locale, value);
    if plus {
        display_value.push('+');
    }
    let label = count_label(locale, noun, value);
    let secondary_inline = secondary_value.map(|secondary| {
        secondary_label.map_or_else(
            || format!("({})", format_count(locale, secondary)),
            |label| {
                let inline_label = label.to_lowercase();
                format!("({} {inline_label})", format_count(locale, secondary))
            },
        )
    });
    let (bg, border, stripe_class) = match stripe {
        StatStripe::Compound => (
            "bg-stat-compound",
            "border-stat-compound-border",
            "border-l-4 border-l-wd-compound",
        ),
        StatStripe::Taxon => (
            "bg-stat-taxon",
            "border-stat-taxon-border",
            "border-l-4 border-l-wd-taxon",
        ),
        StatStripe::Reference => (
            "bg-stat-reference",
            "border-stat-reference-border",
            "border-l-4 border-l-wd-reference",
        ),
        StatStripe::Entries => (
            "bg-stat-total",
            "border-stat-total-border",
            "border-l-4 border-l-wd-entries",
        ),
    };
    rsx! {
        article {
            class: "relative flex min-w-[140px] flex-[1_1_180px] flex-col gap-1 overflow-hidden rounded-xl border p-3 shadow-xs {bg} {border} {stripe_class}",
            div {
                class: "flex items-baseline gap-1.5",
                span {
                    class: "text-stat font-bold leading-tight text-text tabular-nums",
                    "{display_value}"
                }
                if let Some(secondary_text) = secondary_inline.as_ref() {
                    span {
                        class: "text-micro font-medium text-subtle",
                        "{secondary_text}"
                    }
                }
            }
            span {
                class: "truncate text-micro font-semibold uppercase tracking-wider text-subtle",
                "{label}"
            }
        }
    }
}

#[component]
pub fn StatBar() -> Element {
    let locale = crate::hooks::use_locale();
    let explore = use_results_context().explore;
    let entries_arc =
        crate::features::explore::selectors::use_result_arc_selector(explore, |result| {
            result.entries.clone()
        });
    let toolbar_snapshot = use_toolbar_result_snapshot(explore);
    let fallback_stats: Memo<DatasetStats> =
        use_memo(move || DatasetStats::from_entries(entries_arc.read().0.as_ref()));
    let snapshot_ref = toolbar_snapshot.read();
    let fallback_stats_ref = fallback_stats.read();
    let stats = snapshot_ref
        .total_stats
        .as_ref()
        .unwrap_or(&fallback_stats_ref);
    let entries_value = snapshot_ref
        .total_matches
        .or_else(|| snapshot_ref.total_stats.as_ref().map(|s| s.n_entries))
        .unwrap_or(stats.n_entries);
    let entries_unique_value = stats.n_entries_unique;

    rsx! {
        div {
            class: "flex w-full min-w-0 flex-wrap items-stretch justify-center gap-3 px-0 min-h-[70px]",
            role: "group",
            aria_label: "{t(locale, TextKey::DatasetStatistics)}",
            StatBadge {
                value: entries_value,
                secondary_value: (entries_unique_value != entries_value).then_some(entries_unique_value),
                secondary_label: Some(t(locale, TextKey::Unique)),
                noun: CountNoun::Entry,
                plus: false,
                stripe: StatStripe::Entries,
            }
            StatBadge {
                value: stats.n_compounds,
                secondary_value: None,
                secondary_label: None,
                noun: CountNoun::Compound,
                plus: false,
                stripe: StatStripe::Compound,
            }
            StatBadge {
                value: stats.n_taxa,
                secondary_value: None,
                secondary_label: None,
                noun: CountNoun::Taxon,
                plus: false,
                stripe: StatStripe::Taxon,
            }
            StatBadge {
                value: stats.n_references,
                secondary_value: None,
                secondary_label: None,
                noun: CountNoun::Reference,
                plus: false,
                stripe: StatStripe::Reference,
            }
        }
    }
}

#[component]
pub fn CappedRowsNotice() -> Element {
    let locale = crate::hooks::use_locale();
    let explore = use_results_context().explore;
    let toolbar_snapshot = use_toolbar_result_snapshot(explore);

    rsx! {
        if toolbar_snapshot.read().display_capped_rows {
            div {
                class: "mt-2 flex items-center gap-2 rounded-xl border border-warning/35 bg-warning/10 p-2.5 text-ui font-medium text-warning shadow-xs",
                role: "status",
                aria_live: "polite",
                span { class: "text-sm font-bold", "⚠️" },
                span { "{t(locale, TextKey::DisplayCappedHint)}" }
            }
        }
    }
}
