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
use super::scroll_signals::ScrollSignals;
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
    row_height_px: Signal<usize>,
    /// Browser-only scroll state, empty on a native render.
    scroll: ScrollSignals,
}

#[must_use]
pub(super) fn use_results_table_virtualization(
    total_rows: usize,
) -> ResultsTableVirtualizationController {
    let row_height_px = use_signal(|| ROW_HEIGHT_PX_COMFORTABLE);
    let scroll = ScrollSignals::new(TABLE_VIEWPORT_FALLBACK_PX);

    let config = build_virtualization_config(*row_height_px.read());

    // The browser half knows its own first visible row and viewport height;
    // a native render has neither, and derives the height from the row count.
    #[cfg(target_arch = "wasm32")]
    let state = use_virtualization::use_virtualization(
        config,
        total_rows,
        *scroll.first_visible_row.read(),
        *scroll.viewport_height_px.read(),
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
        row_height_px,
        scroll,
    }
}

impl ResultsTableVirtualizationController {
    #[cfg(target_arch = "wasm32")]
    pub(super) fn sync_after_render(&mut self, total_rows: usize) {
        if total_rows == 0 {
            self.scroll.row_height_measured.set(false);
            if *self.row_height_px.read() != ROW_HEIGHT_PX_COMFORTABLE {
                self.row_height_px.set(ROW_HEIGHT_PX_COMFORTABLE);
            }
            return;
        }

        if should_reset_first_visible_row(total_rows, *self.scroll.first_visible_row.read()) {
            self.scroll.first_visible_row.set(0);
            return;
        }

        let current_row_height = *self.row_height_px.read();
        let row_height_for_frame = current_row_height;

        if !*self.scroll.row_height_measured.read() {
            self.scroll.row_height_measured.set(true);
            let scroll_id = self.config.scroll_id;
            let mut row_height_px = self.row_height_px;
            let fallback = current_row_height;
            // Both halves of this are load-bearing, and they pull in opposite
            // directions, which is why it is written out.
            //
            // The closure is forgotten, so it can never be dropped while a frame is
            // still queued against it. Owning it in a signal instead is the obvious
            // tidy-up and it is wrong: unmounting the table would drop the closure
            // while the browser still held a handle to call, and every frame after that
            // throws "closure invoked recursively or after being dropped".
            //
            // The write is fallible, because a forgotten closure outlives the component
            // that owns the signal it writes. Searching again replaces the table and the
            // signal is gone, and `set` unwraps internally -- so an unguarded write turns
            // a row height nobody will read into a panic that takes the app down.
            //
            // Cancelling the frame would answer both at once, but it needs the frame id
            // kept and cancelled on unmount, and there is no unmount hook on this path.
            // What leaks is one closure per table mount, and the measurement runs once
            // per mount.
            if let Some(win) = window() {
                let outer = Closure::wrap(Box::new(move |_time: f64| {
                    let inner = Closure::wrap(Box::new(move || {
                        let measured = scroll_runtime::measure_row_height_px(scroll_id, fallback);
                        if measured != fallback
                            && let Ok(mut slot) = row_height_px.try_write()
                        {
                            *slot = measured;
                        }
                    }) as Box<dyn FnMut()>);
                    if let Some(win) = window() {
                        let _ = win.request_animation_frame(inner.as_ref().unchecked_ref());
                    }
                    inner.forget();
                }) as Box<dyn FnMut(f64)>);
                let _ = win.request_animation_frame(outer.as_ref().unchecked_ref());
                outer.forget();
            }
        }

        // Schedule a frame only during initial attachment/measurement and viewport bootstrap,
        // not on every rerender (which can produce visible jitter while scrolling slowly).
        let should_bootstrap_frame = self.scroll.scroll_host.peek().is_none()
            || *self.scroll.viewport_height_px.read() == TABLE_VIEWPORT_FALLBACK_PX;
        if should_bootstrap_frame {
            self.schedule_scroll_frame(total_rows, row_height_for_frame);
        }
    }

    // A native stub mirroring the WASM mutating `sync_after_render`, kept so the
    // two targets have the same signature and the call sites need no cfg (see
    // the sibling `handle_scroll`). It uses neither `self` nor its argument,
    // because there is no browser to schedule a frame in.
    #[cfg(not(target_arch = "wasm32"))]
    #[allow(clippy::unused_self, clippy::needless_pass_by_ref_mut)]
    pub(super) const fn sync_after_render(&mut self, _total_rows: usize) {}

    // `total_rows` is consumed on WASM only; kept on native for signature parity.
    #[allow(clippy::unused_self)]
    #[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
    pub(super) fn handle_scroll(&self, total_rows: usize) {
        #[cfg(target_arch = "wasm32")]
        self.schedule_scroll_frame(total_rows, *self.row_height_px.read());
    }

    #[cfg(target_arch = "wasm32")]
    fn schedule_scroll_frame(&self, total_rows: usize, row_height_px: usize) {
        let frame = scroll_runtime::ScrollFrameState {
            scroll_host: self.scroll.scroll_host,
            raf_scheduled: self.scroll.raf_scheduled,
            raf_id: self.scroll.raf_id,
        };
        scroll_runtime::schedule_virtual_scroll_frame(
            frame,
            self.config.scroll_id,
            row_height_px,
            total_rows,
            self.scroll.first_visible_row,
            self.scroll.viewport_height_px,
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
#[path = "virtualization_controller/measurement_frame_safety.rs"]
mod measurement_frame_safety;
