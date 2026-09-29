// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Pure table view model: encapsulates all preparation and sorting orchestration.

use super::row_cells::{PreparedRow, prepare_rows};
use super::sort_model::{SortIndexCache, build_sort_index_cache, indices_for_sort};
use crate::models::{Rows, SortState};
use std::sync::Arc;

/// Complete prepared state for rendering a results table.
#[derive(Clone, PartialEq, Debug)]
pub(super) struct TableViewModel {
    /// Pre-formatted row data (derived from entries).
    pub(super) prepared_rows: Arc<[PreparedRow]>,
    /// Index order according to current sort state.
    pub(super) sorted_indices: Arc<[u32]>,
    /// Current sort state (needed for table header and context).
    pub(super) sort_state: SortState,
}

/// Builds a complete table view model from raw entries and sort state.
/// This is the primary boundary: raw data → fully-prepared view model.
/// All preparation and caching logic is encapsulated here.
#[must_use]
#[cfg(test)]
pub(super) fn build_table_view_model(rows: &Rows, sort_state: SortState) -> TableViewModel {
    let prepared_rows = prepare_rows(rows.as_ref());
    let sort_index_cache = build_sort_index_cache(rows.clone());
    let sorted_indices = indices_for_sort(&sort_index_cache, sort_state);

    TableViewModel {
        prepared_rows,
        sorted_indices,
        sort_state,
    }
}

/// Preparation-only step: row text derivation without sort ordering.
/// Separated so that sort-state changes don't re-run the expensive row preparation.
#[derive(Clone, PartialEq, Debug)]
pub(super) struct PreparedTableState {
    pub(super) prepared_rows: Arc<[PreparedRow]>,
    pub(super) sort_cache: SortIndexCache,
}

/// Build only the row preparation and sort-index cache from entries.
/// This should be memoized independently of sort state.
#[must_use]
pub(super) fn prepare_table_state(rows: Rows) -> PreparedTableState {
    let prepared_rows = prepare_rows(rows.as_ref());
    let sort_cache = build_sort_index_cache(rows);
    PreparedTableState {
        prepared_rows,
        sort_cache,
    }
}

/// Build a [`TableViewModel`] from an already-prepared [`PreparedTableState`] and sort state.
/// Only re-runs index selection when sort changes; row preparation is skipped.
#[must_use]
pub(super) fn apply_sort(state: &PreparedTableState, sort: SortState) -> TableViewModel {
    let sorted_indices = indices_for_sort(&state.sort_cache, sort);
    TableViewModel {
        prepared_rows: state.prepared_rows.clone(),
        sorted_indices,
        sort_state: sort,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CompoundEntry, SortColumn, SortDir};
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
}
