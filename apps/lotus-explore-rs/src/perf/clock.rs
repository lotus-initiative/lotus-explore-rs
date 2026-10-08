// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The clock and console edge: the only part of `perf` that differs by target.
//!
//! Everything above this — which labels are open, what happens when one is opened
//! twice, the drop guard — is target-independent and lives in the parent module, so
//! it is exercised by the same tests on both targets. What is left here is a
//! monotonic handle, the two `console.time` calls, and where a log line goes.

use std::time::Duration;

/// What an open timer holds: `performance.now()` on wasm, an [`std::time::Instant`]
/// natively.
#[cfg(target_arch = "wasm32")]
pub(super) type Handle = f64;
#[cfg(not(target_arch = "wasm32"))]
pub(super) type Handle = std::time::Instant;

#[cfg(target_arch = "wasm32")]
fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map_or(0.0, |p| p.now())
}

/// Take a reading to measure elapsed time from.
#[cfg(target_arch = "wasm32")]
pub(super) fn start() -> Handle {
    now_ms()
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn start() -> Handle {
    std::time::Instant::now()
}

/// Open the browser's own timer for `label`.
///
/// The caller has already checked the label is free: `console.time` on a label that
/// is already open logs a warning and does not replace the timer, which is the
/// failure the parent's registry exists to prevent.
#[cfg(target_arch = "wasm32")]
pub(super) fn open(label: &str) {
    web_sys::console::time_with_label(label);
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn open(_label: &str) {}

/// Close the browser's timer for `label`, if it was open.
#[cfg(target_arch = "wasm32")]
pub(super) fn close(label: &str) {
    web_sys::console::time_end_with_label(label);
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn close(_label: &str) {}

/// How long `handle` has been open.
#[cfg(target_arch = "wasm32")]
pub(super) fn elapsed(handle: Handle) -> Duration {
    // Clamped: `performance.now()` is monotonic, but a handle that came back from a
    // dropped scope can read as ahead of "now", and a negative duration in a log line
    // is worse than a zero.
    Duration::from_secs_f64((now_ms() - handle).max(0.0) / 1000.0)
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn elapsed(handle: Handle) -> Duration {
    handle.elapsed()
}

/// Write one already-formatted line to wherever this target logs.
#[cfg(target_arch = "wasm32")]
pub(super) fn log_line(msg: &str) {
    web_sys::console::info_1(&msg.into());
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn log_line(msg: &str) {
    log::info!("{msg}");
}
