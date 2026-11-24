// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

// Two builds share this crate: the browser WASM client and the native test build.
#![cfg_attr(target_arch = "wasm32", allow(clippy::future_not_send))]
#![cfg_attr(
    not(target_arch = "wasm32"),
    allow(
        unused_crate_dependencies,
        reason = "the native non-test binary stub prints usage without calling WASM UI dependencies"
    )
)]
#![allow(
    unreachable_pub,
    reason = "a `pub` item in a target-gated module is unreachable from the other target"
)]
#![allow(
    clippy::empty_enums,
    reason = "Dioxus's `Props` derive emits an empty enum for every `#[component]`; clippy 1.100 reports it at each call site"
)]
#![allow(
    clippy::missing_const_for_fn,
    reason = "fires on the locale-dispatch helpers, which cannot be `const` without cascading into all four locale tables"
)]
#![cfg_attr(test, allow(dead_code))]
//! `lotus-explore-rs` — LOTUS Explorer browser client.

// Dioxus components are PascalCase by convention, so every `#[component]` fn in
// this crate is named `SomeComponent` rather than `some_component`.
#![allow(
    non_snake_case,
    reason = "Dioxus names `#[component]` functions in PascalCase"
)]

#[cfg(any(target_arch = "wasm32", test))]
mod api;
#[cfg(any(target_arch = "wasm32", test))]
mod app;
#[cfg(any(target_arch = "wasm32", test))]
mod app_state;
/// In-browser result cache.
#[cfg(any(target_arch = "wasm32", test))]
mod cache;
#[cfg(any(target_arch = "wasm32", test))]
mod clock;
#[cfg(any(target_arch = "wasm32", test))]
mod components;
#[cfg(any(target_arch = "wasm32", test))]
mod curation;
#[cfg(any(target_arch = "wasm32", test))]
mod document_head;
#[cfg(any(target_arch = "wasm32", test))]
mod download;
#[cfg(any(target_arch = "wasm32", test))]
mod features;
#[cfg(any(target_arch = "wasm32", test))]
mod hooks;
#[cfg(any(target_arch = "wasm32", test))]
mod i18n;
#[cfg(any(target_arch = "wasm32", test))]
mod pages;
#[cfg(any(target_arch = "wasm32", test))]
mod perf;
#[cfg(any(target_arch = "wasm32", test))]
mod repositories;
#[cfg(any(target_arch = "wasm32", test))]
mod services;
#[cfg(any(target_arch = "wasm32", test))]
mod sparql;
#[cfg(any(target_arch = "wasm32", test))]
mod state;
#[cfg(any(target_arch = "wasm32", test))]
mod ui;
#[cfg(any(target_arch = "wasm32", test))]
mod upload;
#[cfg(any(target_arch = "wasm32", test))]
mod vendor_assets;

#[cfg(target_arch = "wasm32")]
use dioxus::prelude::*;

#[cfg(test)]
mod tests;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    // A native build with no server. `dx serve` builds the browser client;
    // `cargo run -p lotus-server` runs the HTTP API.
    eprintln!(
        "lotus-explore-rs (native): this binary builds for the browser (wasm32).\n\
         \n  browser client   dx serve (from apps/lotus-explore-rs)\n\
         \x20 HTTP API        cargo run -p lotus-server --bin lotus-server"
    );
}

#[cfg(target_arch = "wasm32")]
struct ConsoleLogger;

#[cfg(target_arch = "wasm32")]
impl log::Log for ConsoleLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let message = wasm_bindgen::JsValue::from_str(&format!(
            "[{}] {}: {}",
            record.level(),
            record.target(),
            record.args()
        ));
        match record.level() {
            log::Level::Error => web_sys::console::error_1(&message),
            log::Level::Warn => web_sys::console::warn_1(&message),
            log::Level::Info => web_sys::console::info_1(&message),
            log::Level::Debug => web_sys::console::debug_1(&message),
            log::Level::Trace => web_sys::console::log_1(&message),
        }
    }

    fn flush(&self) {}
}

#[cfg(target_arch = "wasm32")]
fn init_browser_log(level: log::Level) {
    static LOGGER: ConsoleLogger = ConsoleLogger;
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(level.to_level_filter());
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    let level = if cfg!(debug_assertions) {
        log::Level::Debug
    } else {
        log::Level::Info
    };
    init_browser_log(level);
    launch(app::shell::AppRoot);
}
