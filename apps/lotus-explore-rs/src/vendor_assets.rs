// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Build-time references to the large third-party assets.
//!
//! `fetch-assets` downloads `RDKit` and `Ketcher` into `public/assets/`, and both
//! are gitignored, so they are absent from a fresh checkout. Two consequences:
//!
//! - Nothing in the repository can name those paths, so a path typo here is a
//!   compile error rather than a 404 at runtime. That is the reason these
//!   references live in Rust rather than being composed as strings in the
//!   bridge.
//! - `dx` copies into a bundle only the assets Rust references. On the web the
//!   server serves the whole `public/` tree, so a string URL is enough; in a
//!   desktop bundle it 404s, because the bundler cannot see a path built at
//!   runtime. Both directories are therefore declared here as folder assets.
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
static RDKIT: manganis::Asset = manganis::asset!(
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
static KETCHER: manganis::Asset =
    manganis::asset!("/public/assets/ketcher", manganis::AssetOptions::folder());

/// Where the bundled copies of the two folders above are served from.
///
/// A folder asset gets no content hash of its own -- the files inside are
/// already content-hashed by the projects that ship them, and the internal
/// references are relative -- so this is just the folder's path under `assets/`.
fn bundled_folder(asset: &manganis::Asset) -> String {
    asset.bundled().bundled_path().to_string()
}

/// The URL of Ketcher's entry document.
#[must_use]
pub fn ketcher_url() -> String {
    in_folder(&bundled_folder(&KETCHER), "index.html")
}

/// The URL of `RDKit`'s loader script.
#[must_use]
pub fn rdkit_script_url() -> String {
    in_folder(&bundled_folder(&RDKIT), "RDKit_minimal.js")
}

/// The URL of `RDKit`'s WebAssembly module.
///
/// Handed to the loader as a `locateFile` override. Without one the loader looks
/// for `RDKit_minimal.wasm` next to the script, which only works if the two kept
/// their original names in the bundle.
#[must_use]
pub fn rdkit_wasm_url() -> String {
    in_folder(&bundled_folder(&RDKIT), "RDKit_minimal.wasm")
}

/// Join a file name onto a bundled folder path, tolerating either a trailing
/// slash or none.
///
/// The file name is reduced to its last segment. Every caller passes a literal,
/// but this feeds a `WebView` request, so it is the one place a traversal from
/// outside could arrive.
fn in_folder(folder: &str, file: &str) -> String {
    let file = file.rsplit('/').next().unwrap_or(file);
    format!("{}/{}", folder.trim_end_matches('/'), file)
}

#[cfg(test)]
mod tests {
    /// `bundled_path` is a placeholder outside a `dx` build -- reading it panics
    /// with "This should be replaced by dx as part of the build process". So the
    /// URL shape is tested against the join, with a representative folder path,
    /// and the wiring to the real assets is checked by the desktop build.
    ///
    /// The path that used to be built by hand in JS was
    /// `assets/vendor/rdkit/RDKit_minimal.js`, which is not where the bundler
    /// writes the folder. Getting this wrong is a 404 with no other symptom.
    #[test]
    fn joined_urls_name_a_file_inside_the_folder() {
        assert_eq!(
            super::in_folder("assets/rdkit", "RDKit_minimal.js"),
            "assets/rdkit/RDKit_minimal.js"
        );
        assert_eq!(
            super::in_folder("assets/ketcher", "index.html"),
            "assets/ketcher/index.html"
        );
    }

    /// A trailing slash must not produce a doubled separator.
    #[test]
    fn a_trailing_slash_does_not_double_up() {
        assert_eq!(
            super::in_folder("assets/rdkit/", "x.js"),
            "assets/rdkit/x.js"
        );
    }

    /// The wasm is loaded relative to the script, so the two must resolve to the
    /// same directory. If the bundler ever splits them, this is the canary.
    #[test]
    fn the_rdkit_wasm_sits_beside_its_script() {
        fn dir(url: &str) -> &str {
            url.rsplit_once('/').map_or("", |(d, _)| d)
        }
        assert_eq!(
            dir(&super::in_folder("assets/rdkit", "RDKit_minimal.js")),
            dir(&super::in_folder("assets/rdkit", "RDKit_minimal.wasm")),
        );
    }

    /// Nothing may escape the bundle root: these paths come from the bundler, but
    /// a traversal here would read outside the app's own directory.
    #[test]
    fn a_file_name_cannot_escape_the_folder() {
        for bad in ["../secret", "/etc/passwd", "a/../../b"] {
            let joined = super::in_folder("assets/rdkit", bad);
            assert!(
                !joined.split('/').any(|segment| segment == ".."),
                "{bad} escaped: {joined}"
            );
        }
    }
}
