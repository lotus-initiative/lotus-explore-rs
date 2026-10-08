// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The signals the browser half of the results table owns.
//!
//! Every field is read by a `requestAnimationFrame` callback, so on a target with
//! no frames they have no meaning and no writer. They were seven `#[cfg]`-gated
//! fields on the controller, meaning seven gated initialisations, seven gated
//! struct-literal entries and seven gated resets to keep in step per field.
//! Grouping them makes the split one field wide.

#[cfg(target_arch = "wasm32")]
use dioxus::prelude::*;

/// Browser-side scroll state. Populated only under wasm.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy)]
pub(super) struct ScrollSignals {
    pub(super) row_height_measured: Signal<bool>,
    pub(super) first_visible_row: Signal<usize>,
    pub(super) viewport_height_px: Signal<usize>,
    pub(super) scroll_host: Signal<Option<web_sys::HtmlElement>>,
    pub(super) raf_scheduled: Signal<bool>,
    pub(super) raf_id: Signal<Option<i32>>,
}

#[cfg(target_arch = "wasm32")]
impl ScrollSignals {
    /// The values before any frame has run: the fallback viewport height, replaced
    /// by the measured one, and no scroll host, bound by the first frame.
    pub(super) fn new(viewport_fallback_px: usize) -> Self {
        Self {
            row_height_measured: use_signal(|| false),
            first_visible_row: use_signal(|| 0usize),
            viewport_height_px: use_signal(|| viewport_fallback_px),
            scroll_host: use_signal(|| None::<web_sys::HtmlElement>),
            raf_scheduled: use_signal(|| false),
            raf_id: use_signal(|| None::<i32>),
        }
    }
}

/// Nothing to hold: a native render has no animation frames to read these from.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy)]
pub(super) struct ScrollSignals;

#[cfg(not(target_arch = "wasm32"))]
impl ScrollSignals {
    pub(super) const fn new(_viewport_fallback_px: usize) -> Self {
        Self
    }
}
