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

/// A deterministic Wikidata QID for a test's label.
///
/// The store only keeps a cell it recognises as a QID -- `Q-Alpha` is not one,
/// and a row keyed by nothing is dropped rather than shown with a broken link --
/// so a fixture has to use numeric ids. Derived from the label so a test states
/// one thing per row instead of inventing an id to go with it.
fn qid_for(prefix: &str, label: &str) -> Arc<str> {
    let mut n: u32 = 2_166_136_261;
    for byte in label.bytes() {
        n = n.wrapping_mul(16_777_619).wrapping_add(u32::from(byte));
    }
    Arc::<str>::from(format!("{prefix}{}", n % 1_000_000 + 1))
}

/// The result set a test states as rows.
///
/// Every test in this file goes through a columnar set, because that is what the
/// table reads. A test that built the view model from anything else would be
/// testing a path that no longer exists.
fn test_set(rows: &[CompoundEntry]) -> Arc<ColumnarResultSet> {
    Arc::new(ColumnarResultSet::from_entries(rows))
}

/// The order and sort state for a given set, with no filters applied.
fn build_table_view_model(set: &Arc<ColumnarResultSet>, sort_state: SortState) -> TableViewModel {
    apply_sort(&prepare_table_state(Arc::clone(set)), sort_state)
}

fn test_entry(
    name: &str,
    mass: Option<f64>,
    formula: Option<&str>,
    taxon_name: &str,
    pub_year: Option<i16>,
    ref_title: Option<&str>,
) -> CompoundEntry {
    CompoundEntry {
        compound_qid: qid_for("Q", name),
        name: Arc::<str>::from(name),
        inchikey: None,
        smiles: None,
        mass,
        formula: formula.map(Arc::<str>::from),
        taxon_qid: qid_for("Q", taxon_name),
        taxon_name: Arc::<str>::from(taxon_name),
        // One reference per row: a title, a DOI and a year belong to the
        // *reference*, so rows sharing one cannot disagree about them.
        reference_qid: qid_for("Q", name),
        reference_node: std::sync::Arc::from(""),
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
    let set = test_set(&rows);
    let sort_state = SortState::default(); // Name, Asc

    let view_model = build_table_view_model(&set, sort_state);

    // Should have prepared all rows (3 entries).
    assert_eq!(view_model.order.len(), 3);
    // Default sort: alphabetical by name ascending => [Alpha, Beta, Gamma] = [1, 2, 0]
    assert_eq!(view_model.order.as_ref(), &[1, 2, 0]);
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
    let set = test_set(&rows);
    let sort_desc = SortState {
        col: SortColumn::Name,
        dir: SortDir::Desc,
    };

    let view_model = build_table_view_model(&set, sort_desc);

    // Descending: [Gamma, Beta, Alpha] = [0, 2, 1]
    assert_eq!(view_model.order.as_ref(), &[0, 2, 1]);
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
    let set = test_set(&rows);
    let sort_by_mass_desc = SortState {
        col: SortColumn::Mass,
        dir: SortDir::Desc,
    };

    let view_model = build_table_view_model(&set, sort_by_mass_desc);

    // Mass descending: 30 > 20 > 10 => [Beta, Gamma, Alpha] = [1, 2, 0]
    assert_eq!(view_model.order.as_ref(), &[1, 2, 0]);
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
    let set = test_set(&rows);
    let sort_state = SortState::default();

    let model1 = build_table_view_model(&set, sort_state);
    let model2 = build_table_view_model(&set, sort_state);

    assert_eq!(model1, model2);
}

#[test]
fn view_model_handles_empty_entries() {
    let rows: Vec<CompoundEntry> = vec![];
    let set = test_set(&rows);
    let sort_state = SortState::default();

    let view_model = build_table_view_model(&set, sort_state);

    assert_eq!(view_model.order.len(), 0);
    assert_eq!(view_model.order.len(), 0);
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
    let set = test_set(&rows);
    let sort_state = SortState::default();

    let view_model = build_table_view_model(&set, sort_state);

    // Prepared rows correspond one-to-one with original entries (before sort reordering).
    assert_eq!(view_model.order.len(), 3);
    // Default sort is Name Asc: [Charlie, Alice, Bob] -> should sort to [Alice, Bob, Charlie]
    // In the original order: Charlie is at 0, Alice is at 1, Bob is at 2
    // So sorted indices should be [1, 2, 0] (Alice, Bob, Charlie)
    assert_eq!(view_model.order.as_ref(), &[1, 2, 0]);
}

// ── Filtering ───────────────────────────────────────────────────────────────

/// Three rows over two taxa and two reference titles, indexed 0 Alpha, 1 Beta,
/// 2 Gamma — so a filtered order can be told apart from a re-sorted one.
fn filter_fixture() -> Vec<CompoundEntry> {
    vec![
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
    ]
}

/// The compound names of `order`, read out of the set the table reads from.
///
/// Deliberately goes through the set rather than through the test's own `rows`:
/// an offset means nothing unless the set agrees on what is at it.
fn shown_as_names(set: &ColumnarResultSet, order: &[u32]) -> Vec<String> {
    order
        .iter()
        .map(|offset| {
            set.entry(*offset as usize)
                .map(|entry| entry.name.to_string())
                .unwrap_or_default()
        })
        .collect()
}

#[test]
fn no_filters_leaves_the_sorted_order_untouched() {
    let rows = filter_fixture();
    let sorted = apply_sort(&prepare_table_state(test_set(&rows)), SortState::default());

    let filtered = filter_order(&sorted.set, &sorted.order, &ColumnFilters::empty());

    assert_eq!(filtered.as_ref(), sorted.order.as_ref());
}

#[test]
fn filtering_drops_rows_without_reordering_the_rest() {
    let rows = filter_fixture();
    let state = prepare_table_state(test_set(&rows));
    let sorted = apply_sort(&state, SortState::default());
    assert_eq!(
        shown_as_names(&state.set, sorted.order.as_ref()),
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
        shown_as_names(&state.set, model.order.as_ref()),
        ["Alpha", "Gamma"]
    );
    assert_eq!(model.visible_row_count(), 2);
}

#[test]
fn filters_apply_under_a_non_default_sort() {
    let rows = filter_fixture();
    let state = prepare_table_state(test_set(&rows));
    let by_mass_desc = SortState {
        col: SortColumn::Mass,
        dir: SortDir::Desc,
    };
    let filters = ColumnFilters {
        formula: "C3".to_owned(),
        ..ColumnFilters::empty()
    };

    let model = apply_sort_and_filters(&state, by_mass_desc, &filters);

    assert_eq!(shown_as_names(&state.set, model.order.as_ref()), ["Gamma"]);
}

#[test]
fn a_numeric_filter_narrows_on_the_column_value() {
    let rows = filter_fixture();
    let state = prepare_table_state(test_set(&rows));

    let filters = ColumnFilters {
        mass_min: Some(15.0),
        mass_max: Some(25.0),
        ..ColumnFilters::empty()
    };
    let model = apply_sort_and_filters(&state, SortState::default(), &filters);
    assert_eq!(shown_as_names(&state.set, model.order.as_ref()), ["Gamma"]);

    let years = ColumnFilters {
        year_min: Some(2002.0),
        ..ColumnFilters::empty()
    };
    let model = apply_sort_and_filters(&state, SortState::default(), &years);
    assert_eq!(
        shown_as_names(&state.set, model.order.as_ref()),
        ["Beta", "Gamma"]
    );
}

#[test]
fn filters_can_exclude_every_row_without_panicking() {
    let rows = filter_fixture();
    let state = prepare_table_state(test_set(&rows));
    let filters = ColumnFilters {
        taxon: "nothing here".to_owned(),
        ..ColumnFilters::empty()
    };

    let model = apply_sort_and_filters(&state, SortState::default(), &filters);

    assert!(model.order.is_empty());
    assert_eq!(model.visible_row_count(), 0);
    // The result set is untouched: filtering is a view over it, not an edit of
    // it, so clearing the filter restores every row without re-fetching anything.
    assert_eq!(
        state.set.row_count(),
        3,
        "an empty view must not have emptied the result set"
    );
}

#[test]
fn the_visible_count_is_the_whole_result_set_when_nothing_is_filtered() {
    let rows = filter_fixture();
    let state = prepare_table_state(test_set(&rows));

    let model = apply_sort_and_filters(&state, SortState::default(), &ColumnFilters::empty());

    assert_eq!(model.visible_row_count(), rows.len());
}

#[test]
fn a_filter_sees_every_row_and_not_just_the_first_screen() {
    // The bug this refactor exists to fix: the query carried a `LIMIT`, so filters
    // only ever saw the rows inside it — a taxon in row 900 was invisible to a filter
    // for it while the toolbar reported the whole set's count. Built well past the old
    // ceiling of 500 with the matching rows at the end: a capped implementation
    // answers 0, an exact one answers 3.
    let mut rows: Vec<CompoundEntry> = (0..900)
        .map(|i| {
            test_entry(
                &format!("Row {i}"),
                Some(100.0),
                Some("C1"),
                "Other",
                Some(2001),
                Some("Ref"),
            )
        })
        .collect();
    for name in ["Late A", "Late B", "Late C"] {
        rows.push(test_entry(
            name,
            Some(100.0),
            Some("C1"),
            "Rosa",
            Some(2020),
            Some("Ref"),
        ));
    }

    let set = test_set(&rows);
    let state = prepare_table_state(Arc::clone(&set));
    let filters = ColumnFilters {
        taxon: "rosa".to_owned(),
        ..ColumnFilters::empty()
    };

    let model = apply_sort_and_filters(&state, SortState::default(), &filters);

    assert_eq!(
        set.row_count(),
        903,
        "the whole result set is held, not a screen of it"
    );
    assert_eq!(
        model.visible_row_count(),
        3,
        "every matching row is found, wherever it sits in the set"
    );
    assert_eq!(
        set.stats().n_entries,
        903,
        "the count covers the whole set, so it cannot disagree with the rows"
    );
}

#[test]
fn an_unfiltered_table_reports_the_whole_result_set() {
    let rows: Vec<CompoundEntry> = (0..750)
        .map(|i| {
            test_entry(
                &format!("Row {i}"),
                Some(1.0),
                Some("C"),
                "T",
                Some(2000),
                Some("R"),
            )
        })
        .collect();
    let set = test_set(&rows);
    let state = prepare_table_state(Arc::clone(&set));

    let model = apply_sort(&state, SortState::default());

    assert_eq!(
        model.visible_row_count(),
        750,
        "750 rows is more than the 500 the old ceiling allowed, and all of them \
         are in the table"
    );
}
