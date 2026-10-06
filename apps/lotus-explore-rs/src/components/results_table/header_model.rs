// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Pure model helpers for sortable results-table headers.

use super::sort_helpers::{aria_sort_for, sort_icon_for};
use crate::i18n::TextKey;
use crate::sort::{SortColumn, SortDir, SortState};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SortableHeaderModel {
    pub(super) col: SortColumn,
    pub(super) label: TextKey,
    pub(super) aria_sort: &'static str,
    pub(super) sort_icon: &'static str,
    pub(super) next_descending: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct HeaderColumnSpec {
    pub(super) col: SortColumn,
    pub(super) label: TextKey,
}

/// The sortable columns, and — deliberately from the same list — the columns
/// that can be filtered. One list, so a filter control cannot end up under a
/// header it does not filter, or be missing for one that needs it.
pub(super) const SORTABLE_COLUMNS: [HeaderColumnSpec; 6] = [
    HeaderColumnSpec {
        col: SortColumn::Name,
        label: TextKey::Compound,
    },
    HeaderColumnSpec {
        col: SortColumn::Mass,
        label: TextKey::Mass,
    },
    HeaderColumnSpec {
        col: SortColumn::Formula,
        label: TextKey::Formula,
    },
    HeaderColumnSpec {
        col: SortColumn::TaxonName,
        label: TextKey::TaxonCol,
    },
    HeaderColumnSpec {
        col: SortColumn::RefTitle,
        label: TextKey::Reference,
    },
    HeaderColumnSpec {
        col: SortColumn::PubYear,
        label: TextKey::Year,
    },
];

#[must_use]
pub(super) fn build_sortable_header_models(
    current_sort: SortState,
) -> [SortableHeaderModel; SORTABLE_COLUMNS.len()] {
    SORTABLE_COLUMNS.map(|spec| SortableHeaderModel {
        col: spec.col,
        label: spec.label,
        aria_sort: aria_sort_for(current_sort, spec.col),
        sort_icon: sort_icon_for(current_sort, spec.col),
        next_descending: next_sort_is_descending(current_sort, spec.col),
    })
}

#[must_use]
fn next_sort_is_descending(sort: SortState, col: SortColumn) -> bool {
    sort.col == col && sort.dir == SortDir::Asc
}

#[cfg(test)]
#[path = "header_model/tests.rs"]
mod tests;
