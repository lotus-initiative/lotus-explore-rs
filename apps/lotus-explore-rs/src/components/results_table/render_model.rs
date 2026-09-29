// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Pure model helpers for the virtualized results-table body.

use super::row_cells::PreparedRow;
use super::table_view_model::TableViewModel;
use crate::hooks::use_virtualization::VirtualizationState;
use crate::models::SortState;
use std::sync::Arc;

#[derive(Clone, PartialEq, Debug)]
pub(super) struct VirtualizedTableRenderModel {
    pub(super) current_sort: SortState,
    pub(super) prepared_rows: Arc<[PreparedRow]>,
    pub(super) sorted_indices: Arc<[u32]>,
    pub(super) start_row: usize,
    pub(super) end_row: usize,
    pub(super) top_spacer_px: usize,
    pub(super) bottom_spacer_px: usize,
}

impl VirtualizedTableRenderModel {
    #[must_use]
    pub(super) const fn has_top_spacer(&self) -> bool {
        self.top_spacer_px > 0
    }

    #[must_use]
    pub(super) const fn has_bottom_spacer(&self) -> bool {
        self.bottom_spacer_px > 0
    }
}

#[must_use]
pub(super) fn build_virtualized_table_render_model(
    view_model: &TableViewModel,
    virtualization: VirtualizationState,
) -> VirtualizedTableRenderModel {
    VirtualizedTableRenderModel {
        current_sort: view_model.sort_state,
        prepared_rows: view_model.prepared_rows.clone(),
        sorted_indices: view_model.sorted_indices.clone(),
        start_row: virtualization.start_row,
        end_row: virtualization.end_row,
        top_spacer_px: virtualization.top_spacer_px,
        bottom_spacer_px: virtualization.bottom_spacer_px,
    }
}

#[cfg(test)]
mod tests {
    use super::super::row_cells::prepare_rows;
    use super::*;
    use crate::hooks::use_virtualization::VirtualizationState;
    use crate::models::{CompoundEntry, Rows, SortColumn, SortDir};

    fn test_entry(name: &str) -> CompoundEntry {
        CompoundEntry {
            compound_qid: Arc::<str>::from(format!("Q-{name}")),
            name: Arc::<str>::from(name),
            inchikey: None,
            smiles: None,
            mass: None,
            formula: None,
            taxon_qid: Arc::<str>::from("T-1"),
            taxon_name: Arc::<str>::from("Taxon"),
            reference_qid: Arc::<str>::from("R-1"),
            ref_title: None,
            ref_doi: None,
            pub_year: None,
            statement: None,
        }
    }

    fn test_view_model(sorted_indices: &[u32], sort_state: SortState) -> TableViewModel {
        let rows: Rows = Arc::from(vec![
            test_entry("Alpha"),
            test_entry("Beta"),
            test_entry("Gamma"),
        ]);
        TableViewModel {
            prepared_rows: prepare_rows(rows.as_ref()),
            sorted_indices: Arc::from(sorted_indices.to_vec().into_boxed_slice()),
            sort_state,
        }
    }

    #[test]
    fn render_model_extracts_visible_window_from_sorted_indices() {
        let view_model = test_view_model(
            &[2, 0, 1],
            SortState {
                col: SortColumn::Name,
                dir: SortDir::Asc,
            },
        );
        let virtualization = VirtualizationState {
            start_row: 1,
            end_row: 3,
            top_spacer_px: 114,
            bottom_spacer_px: 0,
        };

        let render_model = build_virtualized_table_render_model(&view_model, virtualization);

        assert_eq!(render_model.sorted_indices.as_ref(), &[2, 0, 1]);
        assert_eq!(render_model.start_row, 1);
        assert_eq!(render_model.end_row, 3);
        assert_eq!(render_model.top_spacer_px, 114);
        assert_eq!(render_model.bottom_spacer_px, 0);
        assert!(render_model.has_top_spacer());
        assert!(!render_model.has_bottom_spacer());
    }

    #[test]
    fn render_model_handles_empty_visible_window() {
        let view_model = test_view_model(
            &[0, 1, 2],
            SortState {
                col: SortColumn::Name,
                dir: SortDir::Asc,
            },
        );
        let virtualization = VirtualizationState {
            start_row: 9,
            end_row: 9,
            top_spacer_px: 0,
            bottom_spacer_px: 228,
        };

        let render_model = build_virtualized_table_render_model(&view_model, virtualization);

        assert_eq!(render_model.start_row, 9);
        assert_eq!(render_model.end_row, 9);
        assert!(!render_model.has_top_spacer());
        assert!(render_model.has_bottom_spacer());
    }

    #[test]
    fn render_model_preserves_sort_state_and_prepared_row_count() {
        let sort_state = SortState {
            col: SortColumn::PubYear,
            dir: SortDir::Desc,
        };
        let view_model = test_view_model(&[1, 2, 0], sort_state);
        let virtualization = VirtualizationState {
            start_row: 0,
            end_row: 2,
            top_spacer_px: 0,
            bottom_spacer_px: 114,
        };

        let render_model = build_virtualized_table_render_model(&view_model, virtualization);

        assert_eq!(render_model.current_sort, sort_state);
        assert_eq!(render_model.prepared_rows.len(), 3);
        assert_eq!(render_model.sorted_indices.as_ref(), &[1, 2, 0]);
        assert_eq!(render_model.start_row, 0);
        assert_eq!(render_model.end_row, 2);
    }
}
