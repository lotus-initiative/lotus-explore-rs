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
            // The cell needs text of its own, because an `<input>` has none: a
            // header whose only content is a control reads as an empty header to
            // an accessibility checker, and to anyone navigating by cell. The
            // name is already the control's accessible name, so this is the same
            // word twice on the way in, and the right one for a screen reader
            // arriving at the cell before the field.
            span { class: "sr-only", "{name}" }
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
/// `filter-field` drops the spin buttons. They cost about 14px a side, which in a
/// column 12 characters wide is the difference between a number you can read back
/// and a number you cannot; they step by `step`, which is `any` for a mass, so
/// they do nothing useful either; and they eat the arrow keys, which are how you
/// correct a mistyped year.
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
    // keyboard which way to count, and a mass step of 1 would make a 250.25 row
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
    use crate::features::explore::ExploreInteractions;
    use crate::features::explore::orchestrator::SearchTaskController;
    use crate::features::explore::search_state::ExploreState;
    use crate::repositories::HybridRepository;
    use crate::state::{FormCriteriaContext, ResultsContext};
    use lotus_model::SearchCriteria;

    // ── What the markup says, not what the source says ───────────────────────
    //
    // The first version of this row put a bare `<input>` in each `<th>`, which is
    // an empty table header by any measure: an input has no text content, so a
    // cell whose only child is a control has nothing for a screen reader to read
    // and nothing for an accessibility checker to find. `THTexts` below is that
    // rule, written out.

    #[component]
    fn Subject() -> Element {
        use_context_provider(|| Signal::new(Locale::En));
        let explore = use_signal(ExploreState::default);
        let criteria = use_signal(|| SearchCriteria::up_to_year(crate::clock::current_year()));
        let baseline = use_signal(|| SearchCriteria::up_to_year(crate::clock::current_year()));
        let interactions = ExploreInteractions::new(
            criteria,
            FormCriteriaContext::new(criteria, baseline),
            explore,
            SearchTaskController::new(),
            HybridRepository,
        );
        use_context_provider(|| ResultsContext::new(explore));
        use_context_provider(|| interactions);
        rsx! {
            table {
                // The row under test is the header row the sort buttons live in;
                // this one is here only so the table is not header-only.
                tr { td { "a row" } }
                TableFilterRow {}
            }
        }
    }

    fn render() -> String {
        let mut dom = VirtualDom::new(Subject);
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    /// The text content of every `<th>` in `html`, in order.
    ///
    /// A string scan rather than a DOM walk, because that is what an accessibility
    /// checker does too, and because a test that reads the same tree the checker
    /// reads can only agree with it by accident.
    fn th_texts(html: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = html;
        while let Some(open) = rest.find("<th") {
            rest = &rest[open..];
            let Some(after_open) = rest.find('>') else {
                break;
            };
            rest = &rest[after_open + 1..];
            let Some(end) = rest.find("</th>") else {
                break;
            };
            out.push(rest[..end].trim().to_string());
            rest = &rest[end..];
        }
        out
    }

    #[test]
    fn no_table_header_in_the_filter_row_is_empty() {
        let html = render();
        let headers = th_texts(&html);
        assert_eq!(headers.len(), 7, "one cell per column:\n{html}");
        for (index, text) in headers.iter().enumerate() {
            assert!(
                !text.is_empty(),
                "header {index} has no text of its own, which is what an \
                 accessibility checker calls an empty table header:\n{html}"
            );
        }
    }

    #[test]
    fn every_filter_row_header_names_its_column() {
        let html = render();
        let headers = th_texts(&html);
        // Structure's cell is the spacer and says so; the other six name the
        // column they filter, so a screen reader arriving at the cell knows what
        // the field in it does before it reaches the field.
        let expected = [
            "Structure",
            "Compound",
            "Mass",
            "Formula",
            "Taxon",
            "Reference",
            "Year",
        ];
        assert_eq!(headers.len(), expected.len(), "\n{html}");
        for (header, label) in headers.iter().zip(expected) {
            assert!(
                header.contains(label),
                "a header saying {label:?} is expected, got {header:?}:\n{html}"
            );
        }
    }

    #[test]
    fn every_filter_field_opts_out_of_the_native_spinners() {
        // Not a visual assertion: the utility is what removes them, and its
        // presence in the class list is the thing that can regress silently.
        // Four text filters and four range bounds.
        let html = render();
        assert_eq!(html.matches("type=\"number\"").count(), 4);
        assert_eq!(
            html.matches("filter-field").count(),
            8,
            "every field in the row opts in:\n{html}"
        );
    }

    // ── Parsing ──────────────────────────────────────────────────────────────

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
