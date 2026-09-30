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

/// The URL a bundled folder asset is served at.
///
/// `bundled_path` is the name the bundler filed the asset under, which for a
/// folder is just the folder name -- `rdkit`, not `assets/rdkit`. Two things then
/// have to be put right, and both were wrong at once:
///
/// - The `assets/` prefix. `dioxus-desktop` serves a request out of the bundle
///   only when the path starts with `/assets/`; anything else is looked up on
///   disk relative to the working directory and missed.
/// - The leading slash. The document is served at `.../index.html`, so a
///   relative URL resolves against that and becomes
///   `.../index.html/rdkit/RDKit_minimal.js` -- a 404 with a plausible shape.
///
/// A folder asset gets no content hash of its own: the files inside are already
/// content-hashed by the projects that ship them, and their internal references
/// are relative.
fn bundled_folder_url(asset: Option<&manganis::Asset>) -> Option<String> {
    asset.map(asset_url_of)
}

/// [`asset_url_for`], from an asset rather than a path.
fn asset_url_of(asset: &manganis::Asset) -> String {
    asset_url_for(asset.bundled().bundled_path())
}

/// The pure part of [`bundled_folder_url`], so the shape can be tested without
/// a real `dx` build.
fn asset_url_for(bundled: &str) -> String {
    // Only the final segment. The bundler produces a bare folder name, and this
    // is a URL the WebView will request, so a `..` from anywhere must not be
    // able to walk out of the bundle.
    let name = bundled.rsplit('/').next().unwrap_or(bundled);
    if name == ASSET_DIR {
        format!("/{ASSET_DIR}")
    } else {
        format!("/{ASSET_DIR}/{name}")
    }
}

/// Where the desktop bundle serves its assets from.
const ASSET_DIR: &str = "assets";

/// The URL of Ketcher's entry document, or `None` if it was never fetched.
#[must_use]
pub fn ketcher_url() -> Option<String> {
    bundled_folder_url(KETCHER.as_ref()).map(|url| in_folder(&url, "index.html"))
}

/// The URL of `RDKit`'s loader script, or `None` if it was never fetched.
#[must_use]
pub fn rdkit_script_url() -> Option<String> {
    bundled_folder_url(RDKIT.as_ref()).map(|url| in_folder(&url, "RDKit_minimal.js"))
}

/// The URL of `RDKit`'s WebAssembly module.
///
/// Handed to the loader as a `locateFile` override. Without one the loader looks
/// for `RDKit_minimal.wasm` next to the script, which only works if the two kept
/// their original names in the bundle.
#[must_use]
pub fn rdkit_wasm_url() -> Option<String> {
    bundled_folder_url(RDKIT.as_ref()).map(|url| in_folder(&url, "RDKit_minimal.wasm"))
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
    use super::{asset_url_for, in_folder};

    /// `bundled_path` is a placeholder outside a `dx` build -- reading it panics
    /// with "This should be replaced by dx as part of the build process". So the
    /// URL shape is tested against the two pure helpers, and the wiring to the
    /// real assets is checked by `just verify-bundle` against a real bundle.
    #[test]
    fn a_bundled_folder_becomes_a_url_the_resolver_can_serve() {
        // This is the exact 404 that shipped: `bundled_path` is `rdkit`, and a
        // relative URL resolved against `.../index.html` to
        // `.../index.html/rdkit/RDKit_minimal.js`.
        for (bundled, expected) in [
            ("rdkit", "/assets/rdkit"),
            ("ketcher", "/assets/ketcher"),
            // Already prefixed, and already rooted: both must not double up.
            ("assets/rdkit", "/assets/rdkit"),
            ("/assets/rdkit", "/assets/rdkit"),
        ] {
            assert_eq!(asset_url_for(bundled), expected, "for {bundled}");
        }
    }

    /// A path that cannot be served from the bundle must not be turned into one
    /// that looks servable.
    #[test]
    fn a_traversal_cannot_climb_out_of_the_bundle() {
        for input in ["ketcher/../../etc/passwd", "../rdkit", "a/b/../../../c"] {
            let url = asset_url_for(input);
            assert!(!url.contains(".."), "{input} -> {url}");

            let joined = in_folder(&asset_url_for("ketcher"), input);
            assert!(!joined.contains(".."), "{input} -> {joined}");
        }
    }

    #[test]
    fn a_file_is_joined_onto_its_folder() {
        assert_eq!(
            in_folder("/assets/rdkit", "RDKit_minimal.js"),
            "/assets/rdkit/RDKit_minimal.js"
        );
        assert_eq!(
            in_folder("/assets/ketcher", "index.html"),
            "/assets/ketcher/index.html"
        );
    }

    /// A trailing slash must not produce a doubled separator.
    #[test]
    fn a_trailing_slash_does_not_double_up() {
        assert_eq!(in_folder("/assets/rdkit/", "x.js"), "/assets/rdkit/x.js");
    }

    /// The wasm is loaded relative to the script, so the two must resolve to the
    /// same directory. If the bundler ever splits them, this is the canary.
    #[test]
    fn the_rdkit_wasm_sits_beside_its_script() {
        fn dir(url: &str) -> &str {
            url.rsplit_once('/').map_or("", |(d, _)| d)
        }
        assert_eq!(
            dir(&in_folder("/assets/rdkit", "RDKit_minimal.js")),
            dir(&in_folder("/assets/rdkit", "RDKit_minimal.wasm")),
        );
    }
}
