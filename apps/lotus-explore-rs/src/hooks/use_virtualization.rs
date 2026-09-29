// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Virtualization hook encapsulating scroll handling and visible row calculation.

/// Configuration for virtual scrolling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VirtualizationConfig {
    /// Height of each row in pixels.
    pub row_height_px: usize,
    /// Number of extra rows to render outside viewport for smooth scrolling.
    pub overscan_rows: usize,
    /// Fallback viewport height if not yet measured.
    pub viewport_fallback_px: usize,
    /// DOM ID of the scroll container.
    pub scroll_id: &'static str,
}

/// Output state from virtualization hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VirtualizationState {
    /// Index of first visible row.
    pub start_row: usize,
    /// Index of last visible row (exclusive).
    pub end_row: usize,
    /// Top padding in pixels.
    pub top_spacer_px: usize,
    /// Bottom padding in pixels.
    pub bottom_spacer_px: usize,
}

impl VirtualizationState {
    #[cfg(test)]
    #[must_use]
    pub const fn visible_count(self) -> usize {
        self.end_row.saturating_sub(self.start_row)
    }
}

/// Pure window calculation used by the hook and unit tests.
#[must_use]
pub fn compute_virtualization_state(
    config: VirtualizationConfig,
    total_rows: usize,
    first_visible_row: usize,
    viewport_height_px: usize,
) -> VirtualizationState {
    let viewport_height_px = if viewport_height_px == 0 {
        config.viewport_fallback_px
    } else {
        viewport_height_px
    };
    let window_rows = ((viewport_height_px.saturating_add(config.row_height_px - 1))
        / config.row_height_px)
        .max(1)
        .saturating_add(config.overscan_rows * 2);
    let first_row = first_visible_row.min(total_rows);
    let start_row = first_row.saturating_sub(config.overscan_rows);
    let end_row = start_row.saturating_add(window_rows).min(total_rows);
    let top_spacer_px = start_row.saturating_mul(config.row_height_px);
    let bottom_spacer_px = total_rows
        .saturating_sub(end_row)
        .saturating_mul(config.row_height_px);

    VirtualizationState {
        start_row,
        end_row,
        top_spacer_px,
        bottom_spacer_px,
    }
}

/// Hook that manages virtual scrolling state and calculations.
/// Always returns consistent state for SSR compatibility.
#[must_use]
pub fn use_virtualization(
    config: VirtualizationConfig,
    total_rows: usize,
    first_visible_row: usize,
    viewport_height_px: usize,
) -> VirtualizationState {
    compute_virtualization_state(config, total_rows, first_visible_row, viewport_height_px)
}

#[cfg(test)]
mod tests {
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
}
