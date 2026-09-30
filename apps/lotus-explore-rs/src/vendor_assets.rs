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
mod tests {
    use super::{in_folder, last_segment, native_asset_url, web_asset_url};

    /// The published site is at a subpath, `https://lotus.nprod.net/lotus-explore-rs/`,
    /// so a web asset URL must be **relative**. Rooting it broke the deployed site
    /// outright: every asset 404'd and the editor and the toolkit both vanished,
    /// while a local dev server at `/` kept working, so nothing local caught it.
    #[test]
    fn a_web_url_is_relative_so_it_survives_a_subpath() {
        for (bundled, expected) in [
            ("rdkit", "assets/rdkit"),
            ("ketcher", "assets/ketcher"),
            ("assets/rdkit", "assets/rdkit"),
            ("/assets/rdkit", "assets/rdkit"),
        ] {
            let url = web_asset_url(bundled);
            assert_eq!(url, expected, "for {bundled}");
            assert!(
                !url.starts_with('/'),
                "{url} is root-relative and 404s on a site published under a subpath"
            );
        }
    }

    /// A desktop build is the opposite case: the resolver only serves out of the
    /// bundle for a path that starts with `/assets/`, and the document is served
    /// at `.../index.html`, so a relative URL would land under the document's own
    /// path.
    #[test]
    fn a_native_url_is_rooted_under_assets() {
        for (bundled, expected) in [
            ("rdkit", "/assets/rdkit"),
            ("ketcher", "/assets/ketcher"),
            ("assets/rdkit", "/assets/rdkit"),
            ("/assets/rdkit", "/assets/rdkit"),
        ] {
            assert_eq!(native_asset_url(bundled), expected, "for {bundled}");
        }
    }

    /// The editor is loaded in a frame, so the whole thing has to resolve to the
    /// same base whether the file is fetched as a subresource or given to a frame.
    #[test]
    fn a_file_is_joined_onto_its_folder() {
        assert_eq!(
            in_folder(&web_asset_url("ketcher"), "index.html"),
            "assets/ketcher/index.html"
        );
        assert_eq!(
            in_folder(&native_asset_url("rdkit"), "RDKit_minimal.js"),
            "/assets/rdkit/RDKit_minimal.js"
        );
    }

    /// The base is derived from the entry path, so it has to be the directory.
    #[test]
    fn a_base_is_the_directory_of_the_entry_document() {
        let base = |entry: &str| {
            format!(
                "{}/",
                entry
                    .trim_end_matches('/')
                    .rsplit_once('/')
                    .map_or("", |(d, _)| d)
            )
        };
        assert_eq!(base("/assets/ketcher/index.html"), "/assets/ketcher/");
        assert_eq!(base("assets/ketcher/index.html"), "assets/ketcher/");
    }

    /// Nothing may address a file outside the folder, whichever renderer asks.
    #[test]
    fn a_traversal_cannot_climb_out_of_the_bundle() {
        for input in ["ketcher/../../etc/passwd", "../rdkit", "a/b/../../../c"] {
            for url in [web_asset_url(input), native_asset_url(input)] {
                assert!(!url.contains(".."), "{input} escaped: {url}");
            }
        }
        assert_eq!(last_segment("a/b/c"), "c");
    }
}
