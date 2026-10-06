// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Build-time references to the large third-party assets.
//!
//! `fetch-assets` downloads `RDKit` and `Ketcher` into `public/assets/`, and both
//! are gitignored, so they are absent from a fresh checkout. Two consequences,
//! and this module is the meeting point of both:
//!
//! - `dx` copies into a bundle only the assets Rust references. On the web the
//!   server serves the whole `public/` tree, so a string URL is enough; in a
//!   desktop bundle it 404s, because the bundler cannot see a path built at
//!   runtime. Both directories are therefore declared here as folder assets.
//! - A plain `asset!` on a missing directory is a **compile error**, which would
//!   make the crate unbuildable from a bare checkout. That is not hypothetical:
//!   it is what broke every job in CI and the deploy, because those check out
//!   the repository without running `fetch-assets`. So these are `option_asset!`,
//!   which resolves to `None` for a directory that was never fetched.
//!
//! `option_asset!` is not a weaker check than it looks. A path that does not exist
//! yields `None`; a path that exists but is *misspelled* is still a compile error,
//! so a typo cannot slip through as a missing asset. And nothing degrades
//! silently: a `None` means the editor and the toolkit are reported as absent to
//! the user rather than rendering a broken frame or a bridge that never loads.
//!
//! The two are folders rather than individual files because each is a
//! multi-file application: `Ketcher`'s `index.html` loads hashed chunks by a
//! relative path, and `RDKit`'s loader looks for its `.wasm` next to its `.js`.

use dioxus::prelude::*;

/// `RDKit`, the structure toolkit the curation page uses.
///
/// The folder is embedded so `RDKit_minimal.js` and `RDKit_minimal.wasm` both
/// reach the bundle. They must stay in the same directory: the loader resolves
/// the wasm relative to the script, and a hashed filename on one but not the
/// other is exactly the case that breaks.
// Read by the bundler, not by Rust: `asset!` expands to a `&[u8]` read through a
// volatile pointer, which `volatile_composites` rejects, and the shape is fixed
// by the macro.
#[used]
#[allow(dead_code, clippy::volatile_composites)]
static RDKIT: Option<manganis::Asset> = manganis::option_asset!(
    "/public/assets/vendor/rdkit",
    manganis::AssetOptions::folder()
);

/// `Ketcher`, the structure editor, loaded in an iframe.
///
/// A folder for the same reason: `index.html` references `./static/...` chunks
/// by relative path.
// As `RDKIT` above.
#[used]
#[allow(dead_code, clippy::volatile_composites)]
static KETCHER: Option<manganis::Asset> =
    manganis::option_asset!("/public/assets/ketcher", manganis::AssetOptions::folder());

/// Where the desktop bundle serves its assets from.
const ASSET_DIR: &str = "assets";

/// The URL a bundled folder is served at, in a browser.
///
/// **Relative, and that is the whole point.** The site is published under a
/// subpath -- `https://lotus.nprod.net/lotus-explore-rs/` -- so a root-relative
/// `/assets/...` resolves to the domain root, where nothing is published, and
/// every asset 404s. A relative URL resolves against the document, so it is
/// correct at any base path: `/` for a local dev server, `/lotus-explore-rs/`
/// for the deployed site, and a preview build's own.
// Only the browser build asks for this shape, and the tests assert both.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn web_asset_url(bundled: &str) -> String {
    let name = last_segment(bundled);
    if name == ASSET_DIR {
        format!("{ASSET_DIR}/")
    } else {
        format!("{ASSET_DIR}/{name}")
    }
}

/// The URL a bundled folder is served at, in a desktop window.
///
/// **Rooted, and with the `assets/` prefix restored**, which `web_asset_url`
/// deliberately does not need. `dioxus-desktop` serves a request out of the
/// bundle only when the path starts with `/assets/`; anything else is looked up
/// on disk relative to the working directory and missed. The document is served
/// at `.../index.html`, so a relative URL would also resolve to
/// `.../index.html/rdkit/RDKit_minimal.js` and 404.
// Only the native build asks for this shape, and the tests assert both.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn native_asset_url(bundled: &str) -> String {
    let name = last_segment(bundled);
    if name == ASSET_DIR {
        format!("/{ASSET_DIR}/")
    } else {
        format!("/{ASSET_DIR}/{name}")
    }
}

/// The last path segment, so nothing outside the folder can be addressed.
fn last_segment(path: &str) -> &str {
    path.trim_start_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path)
}

/// The bundled folder's URL for the renderer being built.
#[cfg(target_arch = "wasm32")]
fn bundled_folder_url(bundled: &str) -> String {
    web_asset_url(bundled)
}

/// The bundled folder's URL for the renderer being built.
#[cfg(not(target_arch = "wasm32"))]
fn bundled_folder_url(bundled: &str) -> String {
    native_asset_url(bundled)
}

/// The bundled folder's URL, or `None` when it was never fetched.
fn bundled_asset_url(asset: Option<&manganis::Asset>) -> Option<String> {
    asset.map(asset_url_of)
}

/// [`bundled_folder_url`], from an asset rather than a path.
fn asset_url_of(asset: &manganis::Asset) -> String {
    bundled_folder_url(asset.bundled().bundled_path())
}

/// The URL of Ketcher's entry document, or `None` if it was never fetched.
#[must_use]
pub fn ketcher_url() -> Option<String> {
    bundled_asset_url(KETCHER.as_ref()).map(|url| in_folder(&url, "index.html"))
}

/// The URL of `RDKit`'s loader script, or `None` if it was never fetched.
#[must_use]
pub fn rdkit_script_url() -> Option<String> {
    bundled_asset_url(RDKIT.as_ref()).map(|url| in_folder(&url, "RDKit_minimal.js"))
}

/// The URL of `RDKit`'s WebAssembly module.
///
/// Handed to the loader as a `locateFile` override. Without one the loader looks
/// for `RDKit_minimal.wasm` next to the script, which only works if the two kept
/// their original names in the bundle.
#[must_use]
pub fn rdkit_wasm_url() -> Option<String> {
    bundled_asset_url(RDKIT.as_ref()).map(|url| in_folder(&url, "RDKit_minimal.wasm"))
}

/// Join a file name onto a bundled folder path, tolerating either a trailing
/// slash or none.
///
/// The file name is reduced to its last segment. Every caller passes a literal,
/// but this feeds a `WebView` request, so it is the one place a traversal from
/// outside could arrive.
fn in_folder(folder: &str, file: &str) -> String {
    // A file name, not a path: these come from the bundler, but this is a URL
    // into the WebView and the only place a `..` from outside could arrive.
    let file = file.rsplit('/').next().unwrap_or(file);
    format!("{}/{}", folder.trim_end_matches('/'), file)
}

#[cfg(test)]
#[path = "vendor_assets/tests.rs"]
mod tests;
