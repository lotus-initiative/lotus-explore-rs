// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::sort::{SortColumn, SortDir, SortState};

/// ARIA `aria-sort` value for a column header.
pub(super) fn aria_sort_for(state: SortState, col: SortColumn) -> &'static str {
    if state.col != col {
        "none"
    } else if state.dir == SortDir::Asc {
        "ascending"
    } else {
        "descending"
    }
}

pub(super) fn sort_icon_for(state: SortState, col: SortColumn) -> &'static str {
    if state.col == col {
        if state.dir == SortDir::Asc {
            "▴"
        } else {
            "▾"
        }
    } else {
        // Neutral indicator communicates that the column is sortable.
        "⇅"
    }
}
