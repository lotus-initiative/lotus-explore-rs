// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `vendor_assets`, in their own file.

use super::{in_folder, last_segment, native_asset_url, web_asset_url};

/// The published site is at a subpath, `https://lotus-initiative.github.io/lotus-explore-rs/`,
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
