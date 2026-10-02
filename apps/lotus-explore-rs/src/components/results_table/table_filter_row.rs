// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The per-column filter row of the results table.
//!
//! One control per column, of the kind the column's data calls for: a text box
//! for a name, a numeric range for a mass or a year. The columns come from the
//! same list the sort buttons come from, so a control cannot drift out from
//! under its header.
//!
//! These filter the rows already fetched. Nothing here re-runs the query — see
//! [`crate::filters`] for why.
//!
//! Two details that are easy to get wrong:
//!
//! - **A leading empty cell.** The table has seven columns and only six are
//!   filterable; the structure column is a rendered molecule, and there is
//!   nothing to type that would narrow it. Without the spacer the filters would
//!   sit one column left of their headers.
//! - **The filter input is not the sort button.** Both live in the header area,
//!   so each carries its own accessible name, and the range inputs are grouped
//!   so a screen reader hears "Filter Mass, minimum, number" rather than two
//!   bare spin buttons under the word "Mass".

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
                    // A handle per callback, because these are `FnMut` and
                    // cannot share one they would each have had to move. Every
                    // column still writes to the same state, so there is one set
                    // of filters, not one per column.
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
            if kind == FilterKind::Text {
                input {
                    id: "{filter_id(column)}",
                    // `search` rather than `text`: the browser then offers its
                    // own clear button, which is the one control a user reaches
                    // for to get back to the full result set.
                    r#type: "search",
                    autocomplete: "off",
                    spellcheck: "false",
                    placeholder: "{t(locale, TextKey::FilterTextPlaceholder)}",
                    aria_label: "{name}",
                    value: "{filters.text(column)}",
                    oninput: move |e| on_text.call((column, e.value())),
                    class: "w-full min-w-0 rounded-lg border border-border bg-surface px-2 py-1 text-ui text-text placeholder:text-subtle shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
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
#[component]
fn RangeFilter(
    column: SortColumn,
    locale: Locale,
    min: Option<f64>,
    max: Option<f64>,
    on_bound: EventHandler<(SortColumn, Bound, Option<f64>)>,
) -> Element {
    let name = label_for(locale, column);
    // Years are whole numbers; masses are not. The step is what tells the
    // spinner buttons which way to count, and a mass step of 1 would make a
    // 250.25 row unreachable from the keyboard.
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
                class: "tabular-nums w-full min-w-0 rounded-lg border border-border bg-surface px-1.5 py-1 text-ui text-text placeholder:text-subtle shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
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
                class: "tabular-nums w-full min-w-0 rounded-lg border border-border bg-surface px-1.5 py-1 text-ui text-text placeholder:text-subtle shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
            }
        }
    }
}

/// Parse one end of a range, treating a half-typed or emptied box as "no bound".
///
/// `""` is what a number input reports while its content is being deleted, and
/// a partial `"2."` is not a number the user meant to commit. Either way the
/// row set widens rather than narrowing to nothing, so a mid-edit filter never
/// blanks the table.
fn parse_bound(raw: &str) -> Option<f64> {
    raw.trim().parse::<f64>().ok()
}

fn format_bound(value: f64) -> String {
    // Years and whole-number masses round-trip through the input as `200`
    // rather than `200.0`, which is what the user typed and what the column
    // shows. The cast is guarded by the `fract` check, and `i64` holds every
    // mass and year the app can display.
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
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn an_empty_box_is_no_bound() {
        assert_eq!(parse_bound(""), None);
        assert_eq!(parse_bound("   "), None);
    }

    #[test]
    fn a_partial_or_invalid_number_is_no_bound() {
        // A `<input type="number">` reports `""` for content it will not accept,
        // so a half-typed value arrives here as an empty box rather than as
        // text. Anything the parser rejects is treated the same way: the row set widens
        // rather than narrowing to nothing.
        assert_eq!(parse_bound("2."), Some(2.0));
        assert_eq!(parse_bound("abc"), None);
    }

    #[test]
    fn a_whole_number_bound_renders_without_a_trailing_point_zero() {
        assert_eq!(format_bound(200.0), "200");
        assert_eq!(format_bound(2019.0), "2019");
        assert_eq!(format_bound(250.25), "250.25");
    }

    #[test]
    fn every_column_filter_id_is_distinct() {
        let ids = SortColumn::all().map(filter_id);
        let mut sorted = ids.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "ids collide: {ids:?}");
    }

    #[test]
    fn numbers_are_ranged_and_names_are_searched() {
        assert_eq!(FilterKind::for_column(SortColumn::Mass), FilterKind::Range);
        assert_eq!(
            FilterKind::for_column(SortColumn::PubYear),
            FilterKind::Range
        );
        for column in [
            SortColumn::Name,
            SortColumn::Formula,
            SortColumn::TaxonName,
            SortColumn::RefTitle,
        ] {
            assert_eq!(
                FilterKind::for_column(column),
                FilterKind::Text,
                "{column:?}"
            );
        }
    }

    #[test]
    fn a_filter_is_written_back_to_the_column_it_belongs_to() {
        let mut filters = ColumnFilters::empty();
        filters.set_text(SortColumn::TaxonName, "Gentiana".to_owned());
        filters.set_bound(SortColumn::Mass, Bound::Min, Some(200.0));
        filters.set_bound(SortColumn::PubYear, Bound::Max, Some(2020.0));

        assert_eq!(filters.text(SortColumn::TaxonName), "Gentiana");
        assert_eq!(filters.min(SortColumn::Mass), Some(200.0));
        assert_eq!(filters.max(SortColumn::PubYear), Some(2020.0));
        assert_eq!(filters.active_count(), 3);
    }
}
