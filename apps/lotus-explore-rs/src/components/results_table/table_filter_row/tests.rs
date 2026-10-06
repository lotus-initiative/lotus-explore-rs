// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `table_filter_row`, in their own file.

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
// A bare `<input>` in each `<th>` is an empty table header by any measure: an
// input has no text content, so such a cell gives a screen reader nothing to
// read and an accessibility checker nothing to find. `th_texts` below is that
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
            // Only so the table is not header-only.
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
/// checker does too.
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
    // Structure's cell is the spacer and says so; the other six name their
    // column, so a screen reader knows what the field does before reaching it.
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
    // Not a visual assertion: the utility removes them, and its presence in
    // the class list is what regresses silently. Four text, four range bounds.
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
    // A `<input type="number">` reports `""` for content it will not accept, so
    // a half-typed value arrives as an empty box, not as text.
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
