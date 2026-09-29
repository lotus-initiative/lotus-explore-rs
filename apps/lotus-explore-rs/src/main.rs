// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

// This crate compiles Dioxus WASM-client code alongside native-server code in
// a single compilation unit.  On native targets (no `server` feature) main()
// just prints a hint and never launches the Dioxus renderer, so all UI/i18n
// modules are technically unreachable. The following lints are allowed for this
// Dioxus cross-cfg situation: dead_code, unreachable_pub.
// `missing_const_for_fn` (nursery) is allowed crate-wide: it fires ~64× across
// UI/i18n locale-dispatch code where const-ness has no material benefit (the
// dispatchers cannot be `const` without const-cascading into all four locale
// table files, and UI helpers run at runtime only).
// NOTE: `clippy::module_name_repetitions` is deliberately NOT allowed here;
// the few `App*`/`Export*` names that need it carry item-level allows instead.
#![cfg_attr(target_arch = "wasm32", allow(clippy::future_not_send))]
// `tower` serves `server`-feature tests only (`ServiceExt::oneshot`), but Cargo
// has no feature-gated dev-dependencies, so `unused_crate_dependencies`
// false-positives on default-features builds (the canonical `just clippy`
// invocation). The lint stays enabled workspace-wide and remains effective
// for the feature-less `lotus`/`lotus-web-assets` crates.
#![allow(unused_crate_dependencies)]
#![allow(dead_code, unreachable_pub, clippy::missing_const_for_fn)]
//! `lotus-explore-rs` — LOTUS Explorer.

#![allow(non_snake_case)] // Dioxus PascalCase component naming convention

mod api;
mod app;
mod app_state;
/// In-browser result cache (mirrors the native server's result cache).
#[cfg(any(test, target_arch = "wasm32"))]
mod cache;
mod components;
mod curation;
mod document_head;
mod download;
mod export;
mod features;
mod hooks;
mod i18n;
mod models;
mod pages;
mod perf;
mod queries;
mod repositories;
mod services;
mod sparql;
mod state;
mod ui;
mod upload;
mod utils;

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
    eprintln!(
        "lotus-explore-rs (native): enable the `server` feature to host the API \
         (cargo run --features server -p lotus-explore-rs), \
         or build the WASM client with `just serve`."
    );
}

#[cfg(target_arch = "wasm32")]
fn main() {
    let level = if cfg!(debug_assertions) {
        log::Level::Debug
    } else {
        log::Level::Info
    };
    console_log::init_with_level(level).ok();
    launch(app::shell::AppRoot);
}
