// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

// This crate compiles Dioxus WASM-client code alongside native-server code in
// a single compilation unit. On native targets (no `server` feature) main()
// just prints a hint and never launches a renderer, so the modules below are not
// compiled at all there -- see the `cfg` on each one. That is the fix for the
// `dead_code` allow this file used to carry: the lint was right, and the answer
// was to stop building code that has no caller in that configuration, not to
// stop listening. `unreachable_pub` is allowed for the cross-cfg reason --
// `pub` items inside a module that only exists on some targets are unreachable
// from the others -- and `missing_const_for_fn` (nursery) is allowed
// crate-wide: it fires ~64x across UI/i18n locale-dispatch code where
// const-ness has no material benefit (the dispatchers cannot be `const` without
// const-cascading into all four locale table files, and UI helpers run at
// runtime only).
// `dead_code` stays denied and meaningful. Items that are genuinely unused on a
// target are removed or gated there, with a reason at the site; the
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
#![allow(unreachable_pub, clippy::missing_const_for_fn)]
// `dead_code` is denied, and there are three configurations where that is the
// wrong answer. Each is a case where the code in this crate is not the program
// being built, so its items have no caller in *this* binary:
//
// - A test build compiles the app modules so the app's own tests can reach them.
//   No test launches a component tree, so every component and every helper only
//   a component calls is unreferenced.
// - `server` is an HTTP API that shares the query and export modules with the
//   client and renders nothing.
// - `desktop` is a window with no browser to stream a blob into.
//
// The browser build and the default build -- the two where this crate's client
// *is* the program -- keep `dead_code` denied, so a genuinely unused helper is
// still caught there.
#![cfg_attr(
    any(
        test,
        all(feature = "server", not(target_arch = "wasm32")),
        all(feature = "desktop", not(target_arch = "wasm32")),
    ),
    allow(dead_code)
)]
//! `lotus-explore-rs` — LOTUS Explorer.

#![allow(non_snake_case)] // Dioxus PascalCase component naming convention

#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod api;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod app;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod app_state;
/// In-browser result cache (mirrors the native server's result cache).
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod cache;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod cache_key;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod clock;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod components;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod curation;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod document_head;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod download;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod export;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod features;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod hooks;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod i18n;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod pages;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod perf;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod repositories;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod services;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod sort;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod sparql;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod state;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod table_budget;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod ui;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod upload;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod utils;
#[cfg(any(target_arch = "wasm32", feature = "desktop", feature = "server", test))]
mod vendor_assets;

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
    // asking for it: `dx serve` builds the browser client, `--features server`
    // builds the API, `--features desktop` builds the window. The message names
    // all three, because the obvious question when a binary exits immediately
    // is which one was wanted.
    eprintln!(
        "lotus-explore-rs (native): this binary has no user interface.\n\
         \n  browser client   dx serve (from apps/lotus-explore-rs)\n\
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
/// The same `AppRoot` the browser gets, in a `WebView`. This exists because the
/// native build otherwise had no UI path at all: with only the `web` renderer
/// compiled, `dx serve --desktop` built a binary, launched it, and it exited
/// immediately having printed how to build the browser client instead.
///
/// The server feature is a different program -- an HTTP API with no window --
/// and takes precedence, because a deployment that asked for the server wants an
/// API and would be surprised by a `WebView`.
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

    // Dioxus defaults to 800x600. The header is a single row -- title, nav
    // pills, and a four-language switcher -- and at 800 it does not fit, so the
    // switcher is cut off. The explorer is also a results table, which wants
    // width. The window is resizable, so a small screen is cramped rather than
    // broken.
    let window = dioxus_desktop::WindowBuilder::new()
        .with_title("LOTUS Explorer")
        .with_inner_size(dioxus_desktop::LogicalSize::new(1280.0, 860.0))
        .with_resizable(true);

    // The window and dock icon.
    //
    // Left unset, dioxus uses its own placeholder, so the app showed a Dioxus
    // logo in the dock.
    //
    // `icon.icns` rather than the web app's PNG: tao decodes whatever it is
    // given, and the `.icns` is the format macOS asks for. It is the same file
    // the bundler puts in `Contents/Resources` and names in `Info.plist`, so the
    // window, the dock and Finder all agree. The file is `include_bytes!`d
    // rather than read from disk because a window icon is not worth a path that
    // can be wrong at runtime.
    let icon = dioxus_desktop::icon_from_memory::<dioxus_desktop::tao::window::Icon>(
        include_bytes!("../public/icon.icns"),
    )
    .ok();

    let mut config = dioxus_desktop::Config::new()
        .with_data_directory(data_dir)
        .with_window(window);
    if let Some(icon) = icon {
        config = config.with_icon(icon);
    }
    dioxus_desktop::launch::launch(AppRoot, vec![], vec![Box::new(config)]);
}
