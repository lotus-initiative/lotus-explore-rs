// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The per-column filter row of the results table.
//!
//! One control per column, of the kind the column's data calls for: a text box for
//! a name, a numeric range for a mass or a year. The columns come from the same
//! list the sort buttons come from, so a control cannot drift out from under its
//! header. These filter the rows already fetched; nothing here re-runs the query —
//! see [`crate::filters`] for why.
//!
//! - **A leading empty cell.** Seven columns, six filterable: structure is a
//!   rendered molecule with nothing to type. Without the spacer the filters sit one
//!   column left of their headers.
//! - **The filter input is not the sort button.** Both live in the header area, so
//!   each carries its own accessible name, and the range inputs are grouped so a
//!   screen reader hears "Filter Mass, minimum, number" rather than two bare spin
//!   buttons under the word "Mass".

use super::header_model::SORTABLE_COLUMNS;
use crate::features::explore::interactions::use_explore_interactions;
use crate::features::explore::selectors::use_result_selector;
use crate::filters::{Bound, ColumnFilters, FilterKind};
use crate::i18n::{Locale, TextKey, t};
use crate::sort::SortColumn;
use crate::state::use_results_context;
use dioxus::prelude::*;

/// The `id` stem for a column's filter control.
fn filter_id(column: SortColumn) -> String {
    format!("results-filter-{column:?}").to_lowercase()
}

/// "Filter Mass" — the column's own name, so the control says what it narrows.
fn label_for(locale: Locale, column: SortColumn) -> String {
    let column_label = SORTABLE_COLUMNS
        .iter()
        .find(|spec| spec.col == column)
        .map_or("", |spec| t(locale, spec.label));
    format!("{} {column_label}", t(locale, TextKey::FilterColumn))
}

#[component]
pub(super) fn TableFilterRow() -> Element {
    let locale = crate::hooks::use_locale();
    let state = use_results_context();
    let interactions = use_explore_interactions();
    let filters = use_result_selector(state.explore, |result| result.filters.clone());
    let current = filters.read();

    rsx! {
        tr {
            class: "border-b border-shell-border bg-shell-raised text-left",
            th {
                scope: "col",
                class: "px-2 sm:px-3 py-2",
                span { class: "sr-only", "{t(locale, TextKey::Structure)}" }
            }
            for spec in SORTABLE_COLUMNS.iter() {
                {
                    // A handle per callback: these are `FnMut` and cannot share
                    // one. Every column still writes the same state, so there is
                    // one set of filters, not one per column.
                    let text_cell = interactions.clone();
                    let bound_cell = interactions.clone();
                    rsx! {
                        ColumnFilterCell {
                            column: spec.col,
                            locale,
                            filters: current.clone(),
                            on_text: move |(column, value): (SortColumn, String)| {
                                let mut next = filters.peek().clone();
                                next.set_text(column, value);
                                text_cell.set_filters(next);
                            },
                            on_bound: move |(column, bound, value): (SortColumn, Bound, Option<f64>)| {
                                let mut next = filters.peek().clone();
                                next.set_bound(column, bound, value);
                                bound_cell.set_filters(next);
                            },
                        }
                    }
                }
            }
        }
    }
}

/// One column's control, sized by what it filters.
#[component]
fn ColumnFilterCell(
    column: SortColumn,
    locale: Locale,
    filters: ColumnFilters,
    on_text: EventHandler<(SortColumn, String)>,
    on_bound: EventHandler<(SortColumn, Bound, Option<f64>)>,
) -> Element {
    let kind = FilterKind::for_column(column);
    let name = label_for(locale, column);

    rsx! {
        th {
            scope: "col",
            class: "px-2 sm:px-3 py-2 font-normal",
            // The cell needs text of its own, because an `<input>` has none: a
            // header whose only content is a control reads as an empty header to an
            // accessibility checker and to anyone navigating by cell.
            span { class: "sr-only", "{name}" }
            if kind == FilterKind::Text {
                input {
                    id: "{filter_id(column)}",
                    // `search` so the browser offers its own clear button, the
                    // one control that gets the user back to the full result set.
                    r#type: "search",
                    autocomplete: "off",
                    spellcheck: "false",
                    placeholder: "{t(locale, TextKey::FilterTextPlaceholder)}",
                    aria_label: "{name}",
                    value: "{filters.text(column)}",
                    oninput: move |e| on_text.call((column, e.value())),
                    class: "filter-field resize-field min-h-7 w-full min-w-0 rounded-lg border border-border bg-surface px-1.5 py-1 text-ui text-text placeholder:text-subtle shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                }
            } else {
                RangeFilter {
                    column,
                    locale,
                    min: filters.min(column),
                    max: filters.max(column),
                    on_bound,
                }
            }
        }
    }
}

/// A two-box numeric range, grouped so the column's name covers both ends.
///
/// `filter-field` drops the spin buttons: they cost about 14px a side, which in a
/// column 12 characters wide decides whether a number reads back; they step by
/// `step`, which is `any` for a mass; and they eat the arrow keys used to correct a
/// mistyped year.
#[component]
fn RangeFilter(
    column: SortColumn,
    locale: Locale,
    min: Option<f64>,
    max: Option<f64>,
    on_bound: EventHandler<(SortColumn, Bound, Option<f64>)>,
) -> Element {
    let name = label_for(locale, column);
    // Years are whole, masses are not. A mass step of 1 would make a 250.25 row
    // unreachable without retyping it.
    let step = if matches!(column, SortColumn::PubYear) {
        "1"
    } else {
        "any"
    };

    rsx! {
        div {
            class: "flex min-w-0 items-center gap-1",
            role: "group",
            aria_label: "{name}",
            input {
                id: "{filter_id(column)}-min",
                r#type: "number",
                inputmode: "decimal",
                step: "{step}",
                autocomplete: "off",
                placeholder: "{t(locale, TextKey::Min)}",
                aria_label: "{name} {t(locale, TextKey::Min)}",
                value: "{min.map_or_else(String::new, format_bound)}",
                oninput: move |e| on_bound.call((column, Bound::Min, parse_bound(&e.value()))),
                class: "filter-field resize-field min-h-7 tabular-nums w-full min-w-0 rounded-lg border border-border bg-surface px-1 py-1 text-micro text-text placeholder:text-subtle shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
            }
            span { aria_hidden: "true", class: "text-subtle", "-" }
            input {
                id: "{filter_id(column)}-max",
                r#type: "number",
                inputmode: "decimal",
                step: "{step}",
                autocomplete: "off",
                placeholder: "{t(locale, TextKey::Max)}",
                aria_label: "{name} {t(locale, TextKey::Max)}",
                value: "{max.map_or_else(String::new, format_bound)}",
                oninput: move |e| on_bound.call((column, Bound::Max, parse_bound(&e.value()))),
                class: "filter-field resize-field min-h-7 tabular-nums w-full min-w-0 rounded-lg border border-border bg-surface px-1 py-1 text-micro text-text placeholder:text-subtle shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
            }
        }
    }
}

/// Parse one end of a range, treating a half-typed or emptied box as "no bound".
///
/// `""` is what a number input reports mid-deletion, and a partial `"2."` is not
/// a number the user meant to commit. Either way the row set widens rather than
/// narrowing to nothing, so a mid-edit filter never blanks the table.
fn parse_bound(raw: &str) -> Option<f64> {
    raw.trim().parse::<f64>().ok()
}

fn format_bound(value: f64) -> String {
    // `200` rather than `200.0`, which is what the user typed and what the column
    // shows. `i64` holds every mass and year the app can display.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "guarded by `fract() == 0.0`, and both masses and years fit in an i64"
    )]
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

#[cfg(test)]
#[path = "table_filter_row/tests.rs"]
mod tests;
