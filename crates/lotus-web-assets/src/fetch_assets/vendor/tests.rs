// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Tests for the curation vendor cache.
//!
//! The panic lints keep library code from panicking on bad input. A test that
//! fails on a bad fixture is reporting, not panicking.
#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing
)]

use super::*;
use crate::test_support::{MockServer, client, err, http_ok, http_status, temp_dir};

/// An asset with no files, which is enough to exercise the cache decision.
fn asset_at(version: &str) -> VendoredAsset {
    VendoredAsset {
        name: "asset",
        state_key: "asset",
        dir: "asset",
        version: version.to_owned(),
        files: vec![],
    }
}

#[test]
fn an_asset_owns_its_licence_as_well_as_its_payload() {
    let asset = VendoredAsset {
        name: "RDKit",
        state_key: "rdkit",
        dir: "rdkit",
        version: "2026.3.6".to_owned(),
        files: vec![
            (
                "https://example.invalid/a.js".to_owned(),
                "RDKit_minimal.js".to_owned(),
            ),
            (
                "https://example.invalid/licence".to_owned(),
                "LICENSE".to_owned(),
            ),
        ],
    };
    assert_eq!(
        asset.paths(Path::new("public/assets/vendor")),
        vec![
            PathBuf::from("public/assets/vendor/rdkit/RDKit_minimal.js"),
            PathBuf::from("public/assets/vendor/rdkit/LICENSE"),
        ],
        "a missing licence has to invalidate the asset, so it is one of the paths"
    );
}

#[test]
fn an_asset_with_no_files_owns_nothing() {
    let asset = VendoredAsset {
        name: "empty",
        state_key: "empty",
        dir: "empty",
        version: "0".to_owned(),
        files: vec![],
    };
    assert!(asset.paths(Path::new("root")).is_empty());
}

#[test]
fn state_round_trips_both_assets() {
    let raw = "rdkit=2026.3.6\nscholia=1626b5e3\n";
    let parsed = parse_state(raw);
    assert_eq!(parsed.get("rdkit").map(String::as_str), Some("2026.3.6"));
    assert_eq!(parsed.get("scholia").map(String::as_str), Some("1626b5e3"));
    assert_eq!(
        render_state(&parsed),
        raw,
        "format is unchanged, so no re-fetch"
    );
}

#[test]
fn state_ignores_junk_lines_instead_of_failing() {
    let parsed = parse_state("rdkit=1\nnot a pair\n=novalue\n\nscholia=2\n");
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed.get("rdkit").map(String::as_str), Some("1"));
    assert_eq!(parsed.get("scholia").map(String::as_str), Some("2"));
}

#[test]
fn a_version_match_alone_is_not_enough() {
    let asset = asset_at("1.0");
    assert!(
        !asset.is_current(Some("1.0"), false),
        "a partial download leaves the version recorded but files missing"
    );
    assert!(asset.is_current(Some("1.0"), true));
}

#[test]
fn an_unrecorded_asset_is_always_fetched() {
    let asset = asset_at("1.0");
    assert!(!asset.is_current(None, true));
    assert!(!asset.is_current(None, false));
}

#[test]
fn one_stale_asset_leaves_the_other_current() {
    let mut state = parse_state("rdkit=1\nscholia=1\n");
    // Scholia moves; RDKit does not.
    state.insert("scholia".to_owned(), "2".to_owned());
    assert!(
        asset_at("1").is_current(state.get("rdkit").map(String::as_str), true),
        "RDKit must stay cached when only Scholia changed"
    );
    assert!(
        !asset_at("1").is_current(state.get("scholia").map(String::as_str), true),
        "the moved Scholia ref must be re-fetched"
    );
    assert_eq!(
        render_state(&state),
        "rdkit=1\nscholia=2\n",
        "only the moved key is rewritten"
    );
}

#[test]
fn a_cached_asset_is_not_downloaded_again() {
    let dir = temp_dir("vendor-cached");
    let root = dir.join("root");
    fs::create_dir_all(root.join("rdkit")).unwrap_or_else(|e| panic!("{e}"));
    fs::write(root.join("rdkit/RDKit_minimal.js"), "already here")
        .unwrap_or_else(|e| panic!("{e}"));

    let asset = VendoredAsset {
        name: "RDKit",
        state_key: "rdkit",
        dir: "rdkit",
        version: "1".to_owned(),
        files: vec![(
            "https://example.invalid/RDKit_minimal.js".to_owned(),
            "RDKit_minimal.js".to_owned(),
        )],
    };
    let mut state = parse_state("rdkit=1\n");
    // The server has no responses queued, so a fetch would hang and then fail.
    let _server = MockServer::start(vec![]);
    asset
        .refresh(&client(300), &root, &mut state, &dir.join("state"))
        .unwrap_or_else(|e| panic!("a cached asset must not be fetched: {e}"));

    assert_eq!(
        fs::read_to_string(root.join("rdkit/RDKit_minimal.js")).unwrap_or_default(),
        "already here",
        "the cached file is left alone"
    );
    assert_eq!(
        state.get("rdkit").map(String::as_str),
        Some("1"),
        "an unchanged asset keeps its recorded version"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_two_families_are_built_at_the_resolved_versions() {
    let built = assets("2026.3.6".to_owned(), "abc123".to_owned());
    assert_eq!(built[0].state_key, "rdkit");
    assert_eq!(built[1].state_key, "scholia");
    assert_eq!(built[0].version, "2026.3.6");
    assert_eq!(built[1].version, "abc123");
    assert!(
        built[0]
            .files
            .iter()
            .any(|(url, _)| url.contains("@rdkit/rdkit@2026.3.6")),
        "the resolved version is what the download URL is built from: {:?}",
        built[0].files
    );
    assert!(
        built[1]
            .files
            .iter()
            .any(|(url, _)| url.contains("/abc123/")),
        "the resolved commit pins the Scholia raw URL: {:?}",
        built[1].files
    );
}

#[test]
fn a_pinned_rdkit_version_skips_the_metadata_request() {
    // Both asset families are asked for before anything is fetched, so this also
    // pins the order: a failure to resolve must not leave a half-refreshed tree.
    let server = MockServer::start(vec![http_ok(r#"{"version":"2026.9.9"}"#)]);
    let client = client(3000);
    assert_eq!(
        resolve_metadata_version(&client, &server.url("/meta"), "2026.3.6").unwrap_or_default(),
        "2026.3.6"
    );
    assert_eq!(
        resolve_metadata_version(&client, &server.url("/meta"), "latest").unwrap_or_default(),
        "2026.9.9"
    );
    assert!(
        server.next_request().contains("/meta"),
        "only the unresolved one asked the network"
    );
}

#[test]
fn metadata_a_500_is_not_treated_as_an_asset() {
    let server = MockServer::start(vec![http_status(500, "Server Error")]);
    let message = err(resolve_metadata_version(
        &client(3000),
        &server.url("/meta"),
        "latest",
    ));
    assert!(message.contains("500"), "got {message}");
}
