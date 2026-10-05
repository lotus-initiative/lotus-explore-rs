// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::features::explore::use_toolbar_result_snapshot;
use crate::i18n::{CountNoun, TextKey, count_label, format_count, t};
use crate::state::use_results_context;
use dioxus::prelude::*;

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
    let toolbar_snapshot = use_toolbar_result_snapshot(explore);
    let snapshot_ref = toolbar_snapshot.read();
    // The set always carries its own exact counts, so this is never absent in
    // practice: there is no fallback to recompute. `Option` only because the
    // snapshot is shared with the download-only path, where no result was fetched.
    let stats = snapshot_ref.total_stats.clone().unwrap_or_default();
    let entries_value = stats.n_entries;
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
            // "1,234 entries / 800 unique" reads as a bug until you know a row is one
            // compound *as reported by one reference*. The reference column makes the
            // rows differ, so the explanation goes next to the number that differs rather
            // than in a help page nobody opens.
            //
            // A `<details>` rather than a `title` tooltip: there is no hover on a phone,
            // and it stays in the accessibility tree and the document, so a screen reader
            // reaches the same words. Shown only when rows and distinct compounds differ,
            // because a duplicate-free result raising the question invites the reader to
            // go looking for a problem that is not there.
            if entries_unique_value != entries_value {
                details {
                    class: "flex w-full min-w-0 basis-full justify-center",
                    summary {
                        class: "mx-auto cursor-pointer list-none text-center text-sm text-muted underline decoration-dotted underline-offset-4 marker:content-none",
                        "{t(locale, TextKey::DuplicateRowsHint)}"
                    }
                    p {
                        class: "mx-auto mt-1 max-w-[60ch] text-center text-sm leading-relaxed text-muted",
                        "{t(locale, TextKey::DuplicateRowsExplain)}"
                    }
                }
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
