// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Controller hook for results-table virtualization and scroll scheduling.

use super::{
    ROW_HEIGHT_PX_COMFORTABLE, TABLE_SCROLL_ID, TABLE_VIEWPORT_FALLBACK_PX, VIRTUAL_OVERSCAN_ROWS,
};
use crate::hooks::use_virtualization::{self, VirtualizationConfig, VirtualizationState};
use dioxus::prelude::*;

#[cfg(target_arch = "wasm32")]
use super::scroll_runtime;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
#[cfg(target_arch = "wasm32")]
use web_sys::window;

#[derive(Clone)]
pub(super) struct ResultsTableVirtualizationController {
    pub(super) config: VirtualizationConfig,
    pub(super) state: VirtualizationState,
    #[cfg(target_arch = "wasm32")]
    row_height_px: Signal<usize>,
    #[cfg(target_arch = "wasm32")]
    row_height_measured: Signal<bool>,
    #[cfg(target_arch = "wasm32")]
    first_visible_row: Signal<usize>,
    #[cfg(target_arch = "wasm32")]
    viewport_height_px: Signal<usize>,
    #[cfg(target_arch = "wasm32")]
    scroll_host: Signal<Option<web_sys::HtmlElement>>,
    #[cfg(target_arch = "wasm32")]
    scroll_raf_scheduled: Signal<bool>,
    #[cfg(target_arch = "wasm32")]
    scroll_raf_cb: Signal<Option<scroll_runtime::RafClosure>>,
    #[cfg(target_arch = "wasm32")]
    scroll_raf_id: Signal<Option<i32>>,
}

#[must_use]
pub(super) fn use_results_table_virtualization(
    total_rows: usize,
) -> ResultsTableVirtualizationController {
    let row_height_px = use_signal(|| ROW_HEIGHT_PX_COMFORTABLE);
    #[cfg(target_arch = "wasm32")]
    let row_height_measured = use_signal(|| false);
    #[cfg(target_arch = "wasm32")]
    let first_visible_row = use_signal(|| 0usize);
    #[cfg(target_arch = "wasm32")]
    let viewport_height_px = use_signal(|| TABLE_VIEWPORT_FALLBACK_PX);
    #[cfg(target_arch = "wasm32")]
    let scroll_host = use_signal(|| None::<web_sys::HtmlElement>);
    #[cfg(target_arch = "wasm32")]
    let scroll_raf_scheduled = use_signal(|| false);
    #[cfg(target_arch = "wasm32")]
    let scroll_raf_cb = use_signal(|| None::<wasm_bindgen::closure::Closure<dyn FnMut(f64)>>);
    #[cfg(target_arch = "wasm32")]
    let scroll_raf_id = use_signal(|| None::<i32>);

    let config = build_virtualization_config(*row_height_px.read());

    #[cfg(target_arch = "wasm32")]
    let state = use_virtualization::use_virtualization(
        config,
        total_rows,
        *first_visible_row.read(),
        *viewport_height_px.read(),
    );

    #[cfg(not(target_arch = "wasm32"))]
    let state = use_virtualization::use_virtualization(
        config,
        total_rows,
        0,
        server_viewport_height_px(total_rows, *row_height_px.read()),
    );

    ResultsTableVirtualizationController {
        config,
        state,
        #[cfg(target_arch = "wasm32")]
        row_height_px,
        #[cfg(target_arch = "wasm32")]
        row_height_measured,
        #[cfg(target_arch = "wasm32")]
        first_visible_row,
        #[cfg(target_arch = "wasm32")]
        viewport_height_px,
        #[cfg(target_arch = "wasm32")]
        scroll_host,
        #[cfg(target_arch = "wasm32")]
        scroll_raf_scheduled,
        #[cfg(target_arch = "wasm32")]
        scroll_raf_cb,
        #[cfg(target_arch = "wasm32")]
        scroll_raf_id,
    }
}

impl ResultsTableVirtualizationController {
    #[cfg(target_arch = "wasm32")]
    pub(super) fn sync_after_render(&mut self, total_rows: usize) {
        if total_rows == 0 {
            self.row_height_measured.set(false);
            if *self.row_height_px.read() != ROW_HEIGHT_PX_COMFORTABLE {
                self.row_height_px.set(ROW_HEIGHT_PX_COMFORTABLE);
            }
            return;
        }

        if should_reset_first_visible_row(total_rows, *self.first_visible_row.read()) {
            self.first_visible_row.set(0);
            return;
        }

        let current_row_height = *self.row_height_px.read();
        let row_height_for_frame = current_row_height;

        if !*self.row_height_measured.read() {
            self.row_height_measured.set(true);
            let scroll_id = self.config.scroll_id;
            let mut row_height_px = self.row_height_px;
            let fallback = current_row_height;
            if let Some(win) = window() {
                let outer = Closure::wrap(Box::new(move || {
                    let inner = Closure::wrap(Box::new(move || {
                        let measured = scroll_runtime::measure_row_height_px(scroll_id, fallback);
                        if measured != fallback {
                            row_height_px.set(measured);
                        }
                    }) as Box<dyn FnMut()>);
                    if let Some(win) = window() {
                        let _ = win.request_animation_frame(inner.as_ref().unchecked_ref());
                    }
                    inner.forget();
                }) as Box<dyn FnMut()>);
                let _ = win.request_animation_frame(outer.as_ref().unchecked_ref());
                outer.forget();
            }
        }

        // Schedule a frame only during initial attachment/measurement and viewport bootstrap,
        // not on every rerender (which can produce visible jitter while scrolling slowly).
        let should_bootstrap_frame = self.scroll_host.peek().is_none()
            || *self.viewport_height_px.read() == TABLE_VIEWPORT_FALLBACK_PX;
        if should_bootstrap_frame {
            self.schedule_scroll_frame(total_rows, row_height_for_frame);
        }
    }

    #[allow(clippy::unused_self)]
    #[allow(clippy::needless_pass_by_ref_mut)]
    // Native stub mirroring the WASM mutating `sync_after_render` for
    // signature parity on the non-browser target (see sibling `handle_scroll`).
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) const fn sync_after_render(&mut self, _total_rows: usize) {}

    #[allow(clippy::unused_self)]
    // `total_rows` is consumed on WASM only; kept on native for signature parity.
    #[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
    pub(super) fn handle_scroll(&self, total_rows: usize) {
        #[cfg(target_arch = "wasm32")]
        self.schedule_scroll_frame(total_rows, *self.row_height_px.read());
    }

    #[cfg(target_arch = "wasm32")]
    fn schedule_scroll_frame(&self, total_rows: usize, row_height_px: usize) {
        let frame = scroll_runtime::ScrollFrameState {
            scroll_host: self.scroll_host,
            raf_scheduled: self.scroll_raf_scheduled,
            raf_cb: self.scroll_raf_cb,
            raf_id: self.scroll_raf_id,
        };
        scroll_runtime::schedule_virtual_scroll_frame(
            frame,
            self.config.scroll_id,
            row_height_px,
            total_rows,
            self.first_visible_row,
            self.viewport_height_px,
        );
    }
}

#[must_use]
pub(super) const fn build_virtualization_config(row_height_px: usize) -> VirtualizationConfig {
    VirtualizationConfig {
        row_height_px,
        overscan_rows: VIRTUAL_OVERSCAN_ROWS,
        viewport_fallback_px: TABLE_VIEWPORT_FALLBACK_PX,
        scroll_id: TABLE_SCROLL_ID,
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub(super) fn server_viewport_height_px(total_rows: usize, row_height_px: usize) -> usize {
    total_rows
        .saturating_mul(row_height_px)
        .max(TABLE_VIEWPORT_FALLBACK_PX)
}

#[cfg(any(target_arch = "wasm32", test))]
#[must_use]
pub(super) fn should_reset_first_visible_row(total_rows: usize, first_visible_row: usize) -> bool {
    total_rows == 0 && first_visible_row != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtualization_config_uses_results_table_defaults() {
        let config = build_virtualization_config(144);

        assert_eq!(config.row_height_px, 144);
        assert_eq!(config.overscan_rows, VIRTUAL_OVERSCAN_ROWS);
        assert_eq!(config.viewport_fallback_px, TABLE_VIEWPORT_FALLBACK_PX);
        assert_eq!(config.scroll_id, TABLE_SCROLL_ID);
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn server_viewport_height_never_drops_below_fallback() {
        assert_eq!(
            server_viewport_height_px(0, 114),
            TABLE_VIEWPORT_FALLBACK_PX
        );
        assert_eq!(
            server_viewport_height_px(1, 114),
            TABLE_VIEWPORT_FALLBACK_PX
        );
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn server_viewport_height_scales_with_large_datasets() {
        assert_eq!(server_viewport_height_px(10, 114), 1_140);
    }

    #[test]
    fn empty_dataset_resets_scrolled_position_only_when_needed() {
        assert!(should_reset_first_visible_row(0, 5));
        assert!(!should_reset_first_visible_row(0, 0));
        assert!(!should_reset_first_visible_row(3, 5));
    }
}
