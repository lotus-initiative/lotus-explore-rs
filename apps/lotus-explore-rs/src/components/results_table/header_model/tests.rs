// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `header_model`, in their own file.

#![allow(clippy::expect_used)]

use super::*;

#[test]
fn active_ascending_column_switches_to_descending_on_next_toggle() {
    let models = build_sortable_header_models(SortState {
        col: SortColumn::Mass,
        dir: SortDir::Asc,
    });

    let mass = models
        .into_iter()
        .find(|model| model.col == SortColumn::Mass)
        .expect("mass header should be present");

    assert_eq!(mass.aria_sort, "ascending");
    assert_eq!(mass.sort_icon, "▴");
    assert!(mass.next_descending);
}

#[test]
fn inactive_columns_report_neutral_sort_state() {
    let models = build_sortable_header_models(SortState {
        col: SortColumn::Mass,
        dir: SortDir::Desc,
    });

    let name = models
        .into_iter()
        .find(|model| model.col == SortColumn::Name)
        .expect("name header should be present");

    assert_eq!(name.aria_sort, "none");
    assert_eq!(name.sort_icon, "⇅");
    assert!(!name.next_descending);
}

#[test]
fn descending_active_column_stays_non_descending_for_aria_prompt() {
    let models = build_sortable_header_models(SortState {
        col: SortColumn::RefTitle,
        dir: SortDir::Desc,
    });

    let reference = models
        .into_iter()
        .find(|model| model.col == SortColumn::RefTitle)
        .expect("reference header should be present");

    assert_eq!(reference.aria_sort, "descending");
    assert_eq!(reference.sort_icon, "▾");
    assert!(!reference.next_descending);
}
