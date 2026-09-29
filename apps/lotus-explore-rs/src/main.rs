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
// `dead_code` is allowed because the lint does not follow Dioxus's macro output.
// `#[component]` and `#[derive(Routable)]` generate the calls that reach the app
// -- `main` -> `AppBootstrap` -> `AppShell` -> `Route` -> every screen -- and
// rustc's reachability pass works on the un-expanded source. Without this it
// reports `enum Route is never used` and 540-odd others, all of which the
// compiler would reject if they really were unreachable. It is a false positive
// about generated code, not a licence to leave unused helpers behind: the
// `#[allow(dead_code)]`s inside the i18n dispatch macros are scoped to what
// those macros generate, which is the only place it is needed.
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
mod cache_key;
mod clock;
mod components;
mod curation;
mod document_head;
mod download;
mod export;
mod features;
mod hooks;
mod i18n;
mod pages;
mod perf;
mod repositories;
mod services;
mod sort;
mod sparql;
mod state;
mod table_budget;
mod ui;
mod upload;
mod utils;

#[cfg(all(feature = "server", not(target_arch = "wasm32")))]
mod server;

#[cfg(target_arch = "wasm32")]
use dioxus::prelude::*;

#[cfg(all(not(target_arch = "wasm32"), feature = "desktop"))]
use app::shell::AppRoot;

#[cfg(test)]
mod tests;

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    server::run().await
}

#[cfg(all(
    not(target_arch = "wasm32"),
    not(feature = "server"),
    not(feature = "desktop")
))]
fn main() {
    // A native build with no renderer and no server. This is reachable only by
    // asking for it: `just serve` builds the browser client, `--features server`
    // builds the API, `--features desktop` builds the window. The message names
    // all three, because the obvious question when a binary exits immediately
    // is which one was wanted.
    eprintln!(
        "lotus-explore-rs (native): this binary has no user interface.\n\
         \n  browser client   just serve\n\
         \x20 native window   cargo run --features desktop -p lotus-explore-rs\n\
         \x20 HTTP API        cargo run --features server -p lotus-explore-rs"
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

/// Send `log` records to stderr for the desktop build.
///
/// A no-op when the server feature has already installed a subscriber, so a
/// desktop binary that also enables the server does not install two.
#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "desktop",
    not(feature = "server")
))]
fn tracing_subscriber_init(level: log::Level) {
    // `tracing` bridges `log`; without a subscriber a `log` record goes nowhere,
    // which on a desktop build means the app appears to do nothing on startup.
    use tracing_subscriber::{EnvFilter, fmt};
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level.as_str()));
    let _ = fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

/// A native window, for `dx serve --desktop` and a `--release` binary.
///
/// The same `AppRoot` the browser gets, in a WebView. This exists because the
/// native build otherwise had no UI path at all: with only the `web` renderer
/// compiled, `dx serve --desktop` built a binary, launched it, and it exited
/// immediately having printed how to build the browser client instead.
///
/// The server feature is a different program -- an HTTP API with no window --
/// and takes precedence, because a deployment that asked for the server wants an
/// API and would be surprised by a WebView.
#[cfg(all(
    not(target_arch = "wasm32"),
    not(feature = "server"),
    feature = "desktop"
))]
fn main() {
    let level = if cfg!(debug_assertions) {
        log::Level::Debug
    } else {
        log::Level::Info
    };
    // `log` rather than `env_logger`: the logger is a server-feature
    // dependency, and a desktop build should not need the API's dependency tree
    // to print a line. Dioxus installs a `log` bridge, so a backend is still
    // wanted; `tracing` is already in the tree for the same reason.
    tracing_subscriber_init(level);

    // `dioxus_desktop::launch::launch` takes the root component, any root
    // contexts, and the platform config. The app has no root contexts --
    // `AppProviders` builds them in the component tree -- so that is empty.
    //
    // The data directory is the app's own, not the temp directory, so a
    // desktop session keeps its locale and dark-mode choice. A preference that
    // resets itself when the OS clears temp is worse than one never stored.
    let data_dir = dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("lotus-explore-rs");
    let config = dioxus_desktop::Config::new().with_data_directory(data_dir);
    dioxus_desktop::launch::launch(AppRoot, vec![], vec![Box::new(config)]);
}
