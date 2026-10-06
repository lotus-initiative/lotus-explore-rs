// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `use_virtualization`, in their own file.

use super::*;

fn config() -> VirtualizationConfig {
    VirtualizationConfig {
        row_height_px: 100,
        overscan_rows: 2,
        viewport_fallback_px: 500,
        scroll_id: "test-scroll",
    }
}

#[test]
fn zero_rows_return_empty_window() {
    let state = compute_virtualization_state(config(), 0, 0, 0);
    assert_eq!(
        state,
        VirtualizationState {
            start_row: 0,
            end_row: 0,
            top_spacer_px: 0,
            bottom_spacer_px: 0,
        }
    );
    assert_eq!(state.visible_count(), 0);
}

#[test]
fn fallback_viewport_is_used_when_measurement_is_zero() {
    let state = compute_virtualization_state(config(), 100, 0, 0);
    assert_eq!(state.start_row, 0);
    assert_eq!(state.end_row, 9);
    assert_eq!(state.top_spacer_px, 0);
    assert_eq!(state.bottom_spacer_px, 9_100);
    assert_eq!(state.visible_count(), 9);
}

#[test]
fn overscan_window_clamps_at_dataset_end() {
    let state = compute_virtualization_state(config(), 10, 8, 200);
    assert_eq!(state.start_row, 6);
    assert_eq!(state.end_row, 10);
    assert_eq!(state.top_spacer_px, 600);
    assert_eq!(state.bottom_spacer_px, 0);
    assert_eq!(state.visible_count(), 4);
}

#[test]
fn first_visible_row_is_clamped_to_total_rows() {
    let state = compute_virtualization_state(config(), 3, 999, 200);
    assert_eq!(state.start_row, 1);
    assert_eq!(state.end_row, 3);
    assert_eq!(state.top_spacer_px, 100);
    assert_eq!(state.bottom_spacer_px, 0);
    assert_eq!(state.visible_count(), 2);
}
