// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Performance monitoring and logging infrastructure for SPARQL query execution.

use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

/// Timer handle - stores platform-specific timing data.
/// On WASM, this stores `performance.now()` milliseconds.
#[cfg(target_arch = "wasm32")]
pub type TimerHandle = f64;

#[cfg(not(target_arch = "wasm32"))]
pub type TimerHandle = Instant;

#[cfg(target_arch = "wasm32")]
fn wasm_now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map_or(0.0, |p| p.now())
}

/// Log a message with timing context. Works cross-platform (WASM console vs. native stdout).
pub fn log_timing(phase: &str, message: &str, duration: Option<Duration>) {
    if !log::log_enabled!(log::Level::Info) {
        return;
    }

    let msg = duration.map_or_else(
        || format!("[LOTUS:{phase}] {message}"),
        |d| {
            let ms = d.as_secs_f64() * 1000.0;
            format!("[LOTUS:{phase}] {message} ({ms:.1}ms)")
        },
    );

    #[cfg(target_arch = "wasm32")]
    {
        web_sys::console::info_1(&msg.into());
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        log::info!("{msg}");
    }
}

/// Start a `console.time()` block on WASM, or return the current timestamp on native.
#[cfg(target_arch = "wasm32")]
pub fn start_timer(label: &str) -> TimerHandle {
    web_sys::console::time_with_label(label);
    wasm_now_ms()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn start_timer(_label: &str) -> TimerHandle {
    Instant::now()
}

/// End a `console.time()` block on WASM and compute elapsed duration on native.
#[cfg(target_arch = "wasm32")]
pub fn end_timer(label: &str, started: TimerHandle) -> Duration {
    web_sys::console::time_end_with_label(label);
    let elapsed_ms = (wasm_now_ms() - started).max(0.0);
    Duration::from_secs_f64(elapsed_ms / 1000.0)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn end_timer(_label: &str, started: TimerHandle) -> Duration {
    started.elapsed()
}
