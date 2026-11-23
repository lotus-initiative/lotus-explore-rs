// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

// Three programs share this crate: the browser client, an HTTP API, and the
// test build. The crate-wide lint exceptions below are the cost of that, and
// each one is scoped to where it is actually wrong.
#![cfg_attr(target_arch = "wasm32", allow(clippy::future_not_send))]
// `tower` is a `server`-feature test dependency. Cargo cannot express a
// feature-gated dev-dependency, so `unused_crate_dependencies` false-positives
// on a default-features build. The lint stays on workspace-wide, where it still
// catches things in `lotus` and `lotus-web-assets`.
#![allow(unused_crate_dependencies)]
// `pub` inside a module that only exists on some targets is unreachable from the
// others -- a `#[cfg(not(target_arch = "wasm32"))]` module is `pub` for the server
// build and absent for the browser one.
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
// `dead_code` is denied, and allowed only where the code in this crate is not
// the program being built. See the `cfg_attr` below for which those are.
#![cfg_attr(
    any(test, all(feature = "server", not(target_arch = "wasm32"))),
    allow(dead_code)
)]
//! `lotus-explore-rs` — LOTUS Explorer.

// Dioxus components are PascalCase by convention, so every `#[component]` fn in
// this crate is named `SomeComponent` rather than `some_component`.
#![allow(
    non_snake_case,
    reason = "Dioxus names `#[component]` functions in PascalCase"
)]

#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod api;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod app;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod app_state;
/// In-browser result cache (mirrors the native server's result cache).
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod cache;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod cache_key;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod clock;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod components;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod curation;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod document_head;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod download;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod export;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod features;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod hooks;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod i18n;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod pages;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod perf;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod repositories;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod services;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod sparql;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod state;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod ui;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod upload;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod utils;
#[cfg(any(target_arch = "wasm32", feature = "server", test))]
mod vendor_assets;

#[cfg(all(feature = "server", not(target_arch = "wasm32")))]
mod server;

#[cfg(target_arch = "wasm32")]
use dioxus::prelude::*;

#[cfg(test)]
mod tests;

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    server::run().await
}

#[cfg(all(not(target_arch = "wasm32"), not(feature = "server")))]
fn main() {
    // A native build with no server. `dx serve` builds the browser client;
    // `--features server` builds the API.
    eprintln!(
        "lotus-explore-rs (native): this binary has no user interface.\n\
         \n  browser client   dx serve (from apps/lotus-explore-rs)\n\
         \x20 HTTP API        cargo run --features server -p lotus-explore-rs"
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
