// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for the table view model, in their own file.
//!
//! This decides what a reader sees for a given scroll position and row budget,
//! so most of what follows is about the boundary: the first window, the last one,
//! and a budget smaller than a single row.

use super::*;
use crate::filters::ColumnFilters;
use crate::sort::{SortColumn, SortDir, SortState};
use lotus_model::CompoundEntry;
use std::sync::Arc;

fn test_entry(
    name: &str,
    mass: Option<f64>,
    formula: Option<&str>,
    taxon_name: &str,
    pub_year: Option<i16>,
    ref_title: Option<&str>,
) -> CompoundEntry {
    CompoundEntry {
        compound_qid: Arc::<str>::from(format!("Q-{name}")),
        name: Arc::<str>::from(name),
        inchikey: None,
        smiles: None,
        mass,
        formula: formula.map(Arc::<str>::from),
        taxon_qid: Arc::<str>::from(format!("T-{taxon_name}")),
        taxon_name: Arc::<str>::from(taxon_name),
        reference_qid: Arc::<str>::from("R-1"),
        ref_title: ref_title.map(Arc::<str>::from),
        ref_doi: None,
        pub_year,
        statement: None,
    }
}

#[test]
fn view_model_contains_prepared_rows_and_sorted_indices() {
    let rows = vec![
        test_entry(
            "Gamma",
            Some(3.0),
            Some("C3"),
            "Taxon C",
            Some(2003),
            Some("Ref C"),
        ),
        test_entry(
            "Alpha",
            Some(1.0),
            Some("C1"),
            "Taxon A",
            Some(2001),
            Some("Ref A"),
        ),
        test_entry(
            "Beta",
            Some(2.0),
            Some("C2"),
            "Taxon B",
            Some(2002),
            Some("Ref B"),
        ),
    ];
    let rows_arc: Rows = Arc::from(rows);
    let sort_state = SortState::default(); // Name, Asc

    let view_model = build_table_view_model(&rows_arc, sort_state);

    // Should have prepared all rows (3 entries).
    assert_eq!(view_model.prepared_rows.len(), 3);
    // Default sort: alphabetical by name ascending => [Alpha, Beta, Gamma] = [1, 2, 0]
    assert_eq!(view_model.sorted_indices.as_ref(), &[1, 2, 0]);
    // Sort state should be included.
    assert_eq!(view_model.sort_state, sort_state);
}

#[test]
fn view_model_respects_sort_direction() {
    let rows = vec![
        test_entry(
            "Gamma",
            Some(3.0),
            Some("C3"),
            "Taxon C",
            Some(2003),
            Some("Ref C"),
        ),
        test_entry(
            "Alpha",
            Some(1.0),
            Some("C1"),
            "Taxon A",
            Some(2001),
            Some("Ref A"),
        ),
        test_entry(
            "Beta",
            Some(2.0),
            Some("C2"),
            "Taxon B",
            Some(2002),
            Some("Ref B"),
        ),
    ];
    let rows_arc: Rows = Arc::from(rows);
    let sort_desc = SortState {
        col: SortColumn::Name,
        dir: SortDir::Desc,
    };

    let view_model = build_table_view_model(&rows_arc, sort_desc);

    // Descending: [Gamma, Beta, Alpha] = [0, 2, 1]
    assert_eq!(view_model.sorted_indices.as_ref(), &[0, 2, 1]);
}

#[test]
fn view_model_sorts_by_different_columns() {
    let rows = vec![
        test_entry(
            "Alpha",
            Some(10.0),
            Some("C1"),
            "Taxon A",
            Some(2001),
            Some("Ref A"),
        ),
        test_entry(
            "Beta",
            Some(30.0),
            Some("C2"),
            "Taxon B",
            Some(2002),
            Some("Ref B"),
        ),
        test_entry(
            "Gamma",
            Some(20.0),
            Some("C3"),
            "Taxon C",
            Some(2003),
            Some("Ref C"),
        ),
    ];
    let rows_arc: Rows = Arc::from(rows);
    let sort_by_mass_desc = SortState {
        col: SortColumn::Mass,
        dir: SortDir::Desc,
    };

    let view_model = build_table_view_model(&rows_arc, sort_by_mass_desc);

    // Mass descending: 30 > 20 > 10 => [Beta, Gamma, Alpha] = [1, 2, 0]
    assert_eq!(view_model.sorted_indices.as_ref(), &[1, 2, 0]);
}

#[test]
fn view_model_equality_matches_entries_and_sort_state() {
    let rows = vec![
        test_entry(
            "Alpha",
            Some(1.0),
            Some("C1"),
            "Taxon A",
            Some(2001),
            Some("Ref A"),
        ),
        test_entry(
            "Beta",
            Some(2.0),
            Some("C2"),
            "Taxon B",
            Some(2002),
            Some("Ref B"),
        ),
    ];
    let rows_arc: Rows = Arc::from(rows);
    let sort_state = SortState::default();

    let model1 = build_table_view_model(&rows_arc, sort_state);
    let model2 = build_table_view_model(&rows_arc, sort_state);

    assert_eq!(model1, model2);
}

#[test]
fn view_model_handles_empty_entries() {
    let rows: Vec<CompoundEntry> = vec![];
    let rows_arc: Rows = Arc::from(rows);
    let sort_state = SortState::default();

    let view_model = build_table_view_model(&rows_arc, sort_state);

    assert_eq!(view_model.prepared_rows.len(), 0);
    assert_eq!(view_model.sorted_indices.len(), 0);
}

#[test]
fn prepared_rows_appear_in_same_order_as_entries() {
    // Create entries with names that won't be in alphabetical order initially
    let rows = vec![
        test_entry(
            "Charlie",
            Some(3.0),
            Some("C3"),
            "Taxon C",
            Some(2003),
            Some("Ref C"),
        ),
        test_entry(
            "Alice",
            Some(1.0),
            Some("C1"),
            "Taxon A",
            Some(2001),
            Some("Ref A"),
        ),
        test_entry(
            "Bob",
            Some(2.0),
            Some("C2"),
            "Taxon B",
            Some(2002),
            Some("Ref B"),
        ),
    ];
    let rows_arc: Rows = Arc::from(rows);
    let sort_state = SortState::default();

    let view_model = build_table_view_model(&rows_arc, sort_state);

    // Prepared rows correspond one-to-one with original entries (before sort reordering).
    assert_eq!(view_model.prepared_rows.len(), 3);
    // Default sort is Name Asc: [Charlie, Alice, Bob] -> should sort to [Alice, Bob, Charlie]
    // In the original order: Charlie is at 0, Alice is at 1, Bob is at 2
    // So sorted indices should be [1, 2, 0] (Alice, Bob, Charlie)
    assert_eq!(view_model.sorted_indices.as_ref(), &[1, 2, 0]);
}

// ── Filtering ───────────────────────────────────────────────────────────────

/// Three rows over two taxa and two reference titles, indexed 0 Alpha, 1 Beta,
/// 2 Gamma — so a filtered order can be told apart from a re-sorted one.
fn filter_fixture() -> Rows {
    Arc::from(vec![
        test_entry(
            "Alpha",
            Some(10.0),
            Some("C1"),
            "Taxon A",
            Some(2001),
            Some("Ref A"),
        ),
        test_entry(
            "Beta",
            Some(30.0),
            Some("C2"),
            "Taxon B",
            Some(2002),
            Some("Ref B"),
        ),
        test_entry(
            "Gamma",
            Some(20.0),
            Some("C3"),
            "Taxon A",
            Some(2003),
            Some("Ref B"),
        ),
    ])
}

fn shown_as_names(rows: &Rows, order: &[u32]) -> Vec<String> {
    order
        .iter()
        .map(|offset| {
            usize::try_from(*offset)
                .ok()
                .and_then(|index| rows.get(index))
                .map(|entry| entry.name.to_string())
                .unwrap_or_default()
        })
        .collect()
}

#[test]
fn no_filters_leaves_the_sorted_order_untouched() {
    let rows = filter_fixture();
    let sorted = apply_sort(&prepare_table_state(rows.clone()), SortState::default());

    let filtered = filter_indices(
        rows.as_ref(),
        &sorted.sorted_indices,
        &ColumnFilters::empty(),
    );

    assert_eq!(filtered.as_ref(), sorted.sorted_indices.as_ref());
}

#[test]
fn filtering_drops_rows_without_reordering_the_rest() {
    let rows = filter_fixture();
    let state = prepare_table_state(rows.clone());
    let sorted = apply_sort(&state, SortState::default());
    assert_eq!(
        shown_as_names(&rows, sorted.sorted_indices.as_ref()),
        ["Alpha", "Beta", "Gamma"]
    );

    let filters = ColumnFilters {
        taxon: "taxon a".to_owned(),
        ..ColumnFilters::empty()
    };
    let model = apply_sort_and_filters(&state, SortState::default(), &filters);

    // Still alphabetical, not sorted by taxon: the filter removes rows, it does
    // not become the sort key.
    assert_eq!(
        shown_as_names(&rows, model.sorted_indices.as_ref()),
        ["Alpha", "Gamma"]
    );
    assert_eq!(model.visible_row_count(), 2);
}

#[test]
fn filters_apply_under_a_non_default_sort() {
    let rows = filter_fixture();
    let state = prepare_table_state(rows.clone());
    let by_mass_desc = SortState {
        col: SortColumn::Mass,
        dir: SortDir::Desc,
    };
    let filters = ColumnFilters {
        formula: "C3".to_owned(),
        ..ColumnFilters::empty()
    };

    let model = apply_sort_and_filters(&state, by_mass_desc, &filters);

    assert_eq!(
        shown_as_names(&rows, model.sorted_indices.as_ref()),
        ["Gamma"]
    );
}

#[test]
fn a_numeric_filter_narrows_on_the_column_value() {
    let rows = filter_fixture();
    let state = prepare_table_state(rows.clone());

    let filters = ColumnFilters {
        mass_min: Some(15.0),
        mass_max: Some(25.0),
        ..ColumnFilters::empty()
    };
    let model = apply_sort_and_filters(&state, SortState::default(), &filters);
    assert_eq!(
        shown_as_names(&rows, model.sorted_indices.as_ref()),
        ["Gamma"]
    );

    let years = ColumnFilters {
        year_min: Some(2002.0),
        ..ColumnFilters::empty()
    };
    let model = apply_sort_and_filters(&state, SortState::default(), &years);
    assert_eq!(
        shown_as_names(&rows, model.sorted_indices.as_ref()),
        ["Beta", "Gamma"]
    );
}

#[test]
fn filters_can_exclude_every_row_without_panicking() {
    let rows = filter_fixture();
    let state = prepare_table_state(rows);
    let filters = ColumnFilters {
        taxon: "nothing here".to_owned(),
        ..ColumnFilters::empty()
    };

    let model = apply_sort_and_filters(&state, SortState::default(), &filters);

    assert!(model.sorted_indices.is_empty());
    assert_eq!(model.visible_row_count(), 0);
    // The prepared rows are untouched, so clearing the filter restores the table
    // without re-deriving anything.
    assert_eq!(model.prepared_rows.len(), 3);
}

#[test]
fn the_visible_count_is_the_whole_result_set_when_nothing_is_filtered() {
    let rows = filter_fixture();
    let state = prepare_table_state(rows.clone());

    let model = apply_sort_and_filters(&state, SortState::default(), &ColumnFilters::empty());

    assert_eq!(model.visible_row_count(), rows.len());
}
