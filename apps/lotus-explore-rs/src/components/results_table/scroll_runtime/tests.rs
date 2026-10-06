// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `scroll_runtime`, in their own file.

use super::*;

#[test]
fn scroll_top_maps_to_first_visible_row() {
    assert_eq!(next_first_visible_row(0, 100, 50), 0);
    assert_eq!(next_first_visible_row(250, 100, 50), 2);
    assert_eq!(next_first_visible_row(990, 100, 50), 9);
}

#[test]
fn next_first_visible_row_clamps_to_total_rows() {
    assert_eq!(next_first_visible_row(50_000, 100, 7), 7);
}

#[test]
fn zero_row_height_is_safe() {
    assert_eq!(next_first_visible_row(50, 0, 7), 0);
}

#[test]
fn sampled_row_height_falls_back_when_no_rows_are_measured() {
    assert_eq!(resolve_sampled_row_height_px(0, 0, 114), 114);
    assert_eq!(resolve_sampled_row_height_px(0, 0, 0), 1);
}

#[test]
fn sampled_row_height_uses_upward_biased_average() {
    assert_eq!(resolve_sampled_row_height_px(520, 4, 114), 130);
}

#[test]
fn sampled_row_height_never_drops_below_fallback() {
    assert_eq!(resolve_sampled_row_height_px(0, 4, 114), 1);
}
