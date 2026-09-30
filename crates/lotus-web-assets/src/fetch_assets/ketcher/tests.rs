// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Tests for laying the Ketcher archive out on disk.
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

#[test]
fn macos_junk_does_not_hide_the_single_wrapper_directory() {
    // `__MACOSX` is a top-level entry. Counting it makes the archive look like it
    // has two top levels, so nothing is stripped and the editor ends up at
    // `standalone/index.html` instead of `index.html`.
    let names = [
        "__MACOSX/._standalone",
        "standalone/index.html",
        "standalone/static/js/main.js",
    ];
    assert_eq!(
        common_prefix(names.into_iter()),
        Some("standalone".to_owned())
    );
}

#[test]
fn one_top_level_directory_is_stripped_and_several_are_not() {
    let nested = ["standalone/index.html", "standalone/static/js/main.js"];
    assert_eq!(
        common_prefix(nested.into_iter()),
        Some("standalone".to_owned()),
        "a single wrapper directory is stripped so the editor sits at the root"
    );
    // Two top levels: no guess, because the wrong guess writes files to the wrong
    // place and the failure only shows up in the browser.
    assert_eq!(common_prefix(["a/x.js", "b/y.js"].into_iter()), None);
    // Empty and root-only names carry no directory.
    assert_eq!(common_prefix(["/x.js"].into_iter()), None);
    assert_eq!(common_prefix(["/x.js", "/y.js"].into_iter()), None);
    assert_eq!(common_prefix(std::iter::empty()), None);
}

#[test]
fn an_archive_entry_cannot_escape_the_target_directory() {
    // The zip-slip cases. Each of these writes outside `ketcher_dir` if it is
    // joined onto the path unchecked.
    for unsafe_path in [
        "..",
        "../outside.js",
        "static/../../outside.js",
        "/etc/passwd",
        "",
    ] {
        assert!(
            is_unsafe_entry_path(unsafe_path),
            "{unsafe_path:?} must not be written"
        );
    }
    for safe in ["index.html", "static/js/main.js", "a..b.js", "./x.js"] {
        assert!(
            !is_unsafe_entry_path(safe),
            "{safe:?} is inside the directory and must be written"
        );
    }
}

#[test]
fn an_escaping_entry_name_is_refused_outright() {
    // The same rule as above, at the point the decision is actually made.
    assert_eq!(destination_for("standalone/../../etc/passwd", None), None);
    assert_eq!(
        destination_for("standalone/index.html", None),
        Some("standalone/index.html".to_owned())
    );
    assert_eq!(
        destination_for("standalone/static/main.js", Some("standalone")),
        Some("static/main.js".to_owned())
    );
    assert_eq!(
        destination_for("__MACOSX/._x", None),
        None,
        "resource forks are not written"
    );
    assert_eq!(
        destination_for("standalone/closable.abc.js", Some("standalone")),
        None,
        "unused entry bundles are not written"
    );
}

#[test]
fn classifies_macos_junk() {
    assert!(is_macos_junk("__MACOSX"));
    assert!(is_macos_junk("__MACOSX/standalone/._index.html"));
    assert!(is_macos_junk(
        "__MACOSX/standalone/static/js/._duo.546fbaab.js"
    ));
    assert!(is_macos_junk(
        "__MACOSX/standalone/static/js/._asset-manifest.json"
    ));
    assert!(!is_macos_junk("standalone/index.html"));
    assert!(!is_macos_junk("standalone/static/js/main.cb80d824.js"));
    assert!(!is_macos_junk("standalone/static/js/duo.546fbaab.js"));
}

#[test]
fn classifies_unused_entries() {
    assert!(is_unused_entry("standalone/static/js/closable.5cead650.js"));
    assert!(is_unused_entry("standalone/static/js/duo.546fbaab.js"));
    assert!(is_unused_entry("standalone/static/js/popup.ec23766a.js"));
    assert!(is_unused_entry(
        "standalone/static/js/closable.5cead650.js.LICENSE.txt"
    ));
    assert!(is_unused_entry(
        "standalone/static/js/duo.546fbaab.js.LICENSE.txt"
    ));
    assert!(!is_unused_entry("standalone/static/js/main.cb80d824.js"));
    assert!(!is_unused_entry(
        "standalone/static/js/157.7de4e426.chunk.js"
    ));
    assert!(!is_unused_entry(
        "standalone/static/js/622.ed91ac0.chunk.js.LICENSE.txt"
    ));
    assert!(!is_unused_entry("standalone/index.html"));
    assert!(!is_unused_entry("standalone/static/css/main.9cca8bc6.css"));
    assert!(!is_unused_entry("standalone/duo.html"));
    assert!(!is_unused_entry(
        "standalone/static/css/closable.9cca8bc6.css"
    ));
    assert!(!is_unused_entry("standalone/._duo.546fbaab.js"));
}

#[test]
fn the_summary_reports_what_was_written_and_what_was_left_out() {
    let mut tally = Tally::default();
    assert_eq!(
        tally.summary(Path::new("public/assets/ketcher")),
        "  extracted 0 file(s) to public/assets/ketcher",
        "nothing skipped means nothing said about skipping"
    );

    tally.record_extracted();
    tally.record_extracted();
    assert!(
        tally
            .summary(Path::new("out"))
            .contains("extracted 2 file(s)")
    );

    tally.record_skipped(4096);
    let summary = tally.summary(Path::new("out"));
    assert!(summary.contains("skipped 4096 bytes"), "got {summary}");
    assert!(
        summary.contains("closable/duo/popup"),
        "names what was skipped: {summary}"
    );

    tally.record_skipped(1);
    assert!(
        tally
            .summary(Path::new("out"))
            .contains("skipped 4097 bytes"),
        "the bytes accumulate rather than being overwritten"
    );
}

#[test]
fn release_url_points_at_github_releases() {
    assert_eq!(
        release_url("3.18.0"),
        "https://github.com/epam/ketcher/releases/download/v3.18.0/ketcher-standalone-3.18.0.zip"
    );
    assert_eq!(
        release_url("3.10.0"),
        "https://github.com/epam/ketcher/releases/download/v3.10.0/ketcher-standalone-3.10.0.zip"
    );
}

/// A target pointed at `dir`, downloading from a mock server.
fn target_for(dir: &std::path::Path, url: String) -> KetcherTarget {
    KetcherTarget {
        dir: dir.to_owned(),
        requested: "9.9.9".to_owned(),
        url: Some(url),
    }
}

#[test]
fn a_download_that_is_not_found_is_an_error() {
    let dir = temp_dir("ketcher-404");
    let server = MockServer::start(vec![http_status(404, "Not Found")]);
    let message = err(fetch_ketcher(
        &client(3000),
        &target_for(&dir, server.url("/ketcher.zip")),
    ));

    assert!(message.contains("404"), "got {message}");
    assert!(
        !dir.join("index.html").exists(),
        "a 404 must not leave a half-written editor behind"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn an_index_html_naming_another_version_is_replaced() {
    // The cache check reads the editor's own index.html rather than keeping a
    // separate marker, so a stale tree has to be recognised and removed.
    let dir = temp_dir("ketcher-stale");
    fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
    fs::write(dir.join("index.html"), "<title>Ketcher v1.0.0</title>")
        .unwrap_or_else(|e| panic!("{e}"));

    let server = MockServer::start(vec![http_ok("not a zip at all")]);
    let message = err(fetch_ketcher(
        &client(3000),
        &target_for(&dir, server.url("/ketcher.zip")),
    ));

    assert!(
        message.to_lowercase().contains("zip"),
        "a body that is not an archive is an error, got {message}"
    );
    assert!(
        !dir.join("index.html").exists(),
        "the stale editor was removed rather than kept alongside"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_editor_already_present_at_that_version_is_left_alone() {
    let dir = temp_dir("ketcher-current");
    fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
    fs::write(dir.join("index.html"), "<title>Ketcher v9.9.9</title>")
        .unwrap_or_else(|e| panic!("{e}"));

    // No responses queued, so a download would hang and then fail.
    let server = MockServer::start(vec![]);
    fetch_ketcher(&client(300), &target_for(&dir, server.url("/ketcher.zip")))
        .unwrap_or_else(|e| panic!("a current editor must not be re-downloaded: {e}"));

    assert_eq!(
        fs::read_to_string(dir.join("index.html")).unwrap_or_default(),
        "<title>Ketcher v9.9.9</title>",
        "the existing tree is untouched"
    );
    let _ = fs::remove_dir_all(&dir);
}
