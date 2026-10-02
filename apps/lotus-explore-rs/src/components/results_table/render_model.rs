// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Pure model helpers for the virtualized results-table body.

use super::row_cells::PreparedRow;
use super::table_view_model::TableViewModel;
use crate::hooks::use_virtualization::VirtualizationState;
use crate::sort::SortState;
use lotus_model::CompoundEntry;
use std::sync::Arc;

/// The rows on screen, and nothing else.
///
/// `rows` and `prepared_rows` are the visible window, materialised out of the
/// result set; `keys` are the row offsets they came from. The full result stays in
/// the columnar store, so this is the only place a row becomes an object.
#[derive(Clone, PartialEq, Debug)]
pub(super) struct VirtualizedTableRenderModel {
    pub(super) current_sort: SortState,
    pub(super) rows: Arc<[CompoundEntry]>,
    pub(super) prepared_rows: Arc<[PreparedRow]>,
    pub(super) keys: Arc<[u32]>,
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
    // The window is clamped against the *filtered* order, which is what is
    // actually drawn. Spacing off the unfiltered count would leave blank bands
    // whenever a filter was narrowing.
    let start = virtualization.start_row.min(view_model.order.len());
    let end = virtualization
        .end_row
        .min(view_model.order.len())
        .max(start);

    let mut keys = Vec::with_capacity(end - start);
    let mut rows = Vec::with_capacity(end - start);
    let mut prepared_rows = Vec::with_capacity(end - start);
    for offset in view_model.order.get(start..end).unwrap_or_default() {
        let Some(entry) = view_model.set.entry(*offset as usize) else {
            continue;
        };
        prepared_rows.push(PreparedRow::from_entry(&entry));
        rows.push(entry);
        keys.push(*offset);
    }

    VirtualizedTableRenderModel {
        current_sort: view_model.sort_state,
        rows: Arc::from(rows.into_boxed_slice()),
        prepared_rows: Arc::from(prepared_rows.into_boxed_slice()),
        keys: Arc::from(keys.into_boxed_slice()),
        start_row: virtualization.start_row,
        end_row: virtualization.end_row,
        top_spacer_px: virtualization.top_spacer_px,
        bottom_spacer_px: virtualization.bottom_spacer_px,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::use_virtualization::VirtualizationState;
    use crate::sort::{SortColumn, SortDir, SortState};
    use lotus_model::ColumnarResultSet;

    /// A set of three rows with the given names.
    ///
    /// The QIDs are numeric because the store only keeps a cell it recognises as
    /// one: `Q-Alpha` is not a QID, and a row keyed by nothing is dropped rather
    /// than shown with a broken link.
    fn set_of(names: &[&str]) -> Arc<ColumnarResultSet> {
        let rows: Vec<CompoundEntry> = names
            .iter()
            .enumerate()
            .map(|(i, name)| CompoundEntry {
                compound_qid: Arc::<str>::from(format!("Q{}", 100 + i)),
                name: Arc::<str>::from(*name),
                taxon_qid: Arc::<str>::from("Q200"),
                taxon_name: Arc::<str>::from("Taxon"),
                reference_qid: Arc::<str>::from("Q300"),
                ..Default::default()
            })
            .collect();
        Arc::new(ColumnarResultSet::from_entries(&rows))
    }

    fn view_model(order: &[u32], sort_state: SortState) -> TableViewModel {
        TableViewModel {
            set: set_of(&["Alpha", "Beta", "Gamma"]),
            order: Arc::from(order.to_vec().into_boxed_slice()),
            sort_state,
        }
    }

    #[test]
    fn render_model_materialises_only_the_visible_window() {
        // The window is the whole point: the set holds every row, and drawing
        // thirty of three million must not cost three million row derivations.
        let model = view_model(
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

        let render = build_virtualized_table_render_model(&model, virtualization);

        // The order is [2, 0, 1], so display rows 1..3 are result-set offsets 0
        // and 1 -- the first and second rows of the set.
        assert_eq!(render.keys.as_ref(), &[0, 1], "keys are result-set offsets");
        assert_eq!(render.rows.len(), 2, "two rows on screen, not three");
        assert_eq!(render.prepared_rows.len(), 2);
        assert_eq!(
            render.rows.first().map(|r| r.name.to_string()),
            Some("Alpha".into())
        );
        assert_eq!(
            render.rows.get(1).map(|r| r.name.to_string()),
            Some("Beta".into()),
            "the window is the tail of the sorted order"
        );
        assert_eq!(render.start_row, 1);
        assert_eq!(render.top_spacer_px, 114);
        assert!(render.has_top_spacer());
        assert!(!render.has_bottom_spacer());
    }

    #[test]
    fn render_model_handles_a_window_past_the_end_of_the_order() {
        let model = view_model(
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

        let render = build_virtualized_table_render_model(&model, virtualization);

        assert!(render.rows.is_empty(), "there is nothing to draw");
        assert!(render.keys.is_empty());
        assert!(render.has_bottom_spacer());
    }

    #[test]
    fn render_model_clamps_a_window_that_overruns() {
        // The virtualiser computes a window from the scroll position, which can
        // briefly point past the last row. Clamping is what stops that being an
        // out-of-bounds read.
        let model = view_model(
            &[0, 1, 2],
            SortState {
                col: SortColumn::Name,
                dir: SortDir::Asc,
            },
        );
        let virtualization = VirtualizationState {
            start_row: 1,
            end_row: 99,
            top_spacer_px: 0,
            bottom_spacer_px: 0,
        };

        let render = build_virtualized_table_render_model(&model, virtualization);

        assert_eq!(render.rows.len(), 2, "clamped to what exists");
        assert_eq!(render.keys.as_ref(), &[1, 2]);
    }

    #[test]
    fn render_model_preserves_the_sort_state_for_the_header() {
        let sort_state = SortState {
            col: SortColumn::PubYear,
            dir: SortDir::Desc,
        };
        let render = build_virtualized_table_render_model(
            &view_model(&[1, 2, 0], sort_state),
            VirtualizationState {
                start_row: 0,
                end_row: 2,
                top_spacer_px: 0,
                bottom_spacer_px: 114,
            },
        );

        assert_eq!(render.current_sort, sort_state);
        assert_eq!(render.keys.as_ref(), &[1, 2]);
    }
}
