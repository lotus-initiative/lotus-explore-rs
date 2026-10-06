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
#[path = "render_model/tests.rs"]
mod tests;
