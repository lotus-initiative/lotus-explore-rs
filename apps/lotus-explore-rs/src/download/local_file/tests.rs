// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `local_file`, in their own file.

// A test that fails to make its temp dir is reporting, not panicking.
#![allow(clippy::expect_used)]

use std::path::PathBuf;

use super::{download_text_in, unique_path};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lotus-local-file-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// A second export of the same file must not overwrite the first, and the
/// extension has to survive the counter.
#[test]
fn a_repeated_export_does_not_overwrite_the_first() {
    let dir = scratch("repeat");
    assert_eq!(unique_path(&dir, "results.csv"), dir.join("results.csv"));

    std::fs::write(dir.join("results.csv"), b"first").expect("write");
    assert_eq!(
        unique_path(&dir, "results.csv"),
        dir.join("results (2).csv")
    );

    std::fs::write(dir.join("results (2).csv"), b"second").expect("write");
    assert_eq!(
        unique_path(&dir, "results.csv"),
        dir.join("results (3).csv")
    );

    // A name that is already a counter is still checked, not assumed free.
    std::fs::write(dir.join("results (3).csv"), b"third").expect("write");
    assert_eq!(
        unique_path(&dir, "results (3).csv"),
        dir.join("results (3) (2).csv"),
    );

    // No extension: the counter goes after the whole name.
    std::fs::write(dir.join("export"), b"x").expect("write");
    assert_eq!(unique_path(&dir, "export"), dir.join("export (2)"));

    // A dotfile has no stem, so it is treated as a bare name.
    assert_eq!(unique_path(&dir, ".gitignore"), dir.join(".gitignore"));

    assert_eq!(
        std::fs::read(dir.join("results.csv")).expect("read"),
        b"first",
        "the first export must be left intact"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// The end-to-end path: a written file exists, holds the content, and the
/// caller is told where it went.
#[test]
fn a_written_export_reports_its_path_and_keeps_its_content() {
    let dir = scratch("write");

    let path = download_text_in(&dir, "id,name\nQ153,Ethanol\n", "results.csv").expect("write");
    assert_eq!(path, dir.join("results.csv"), "the path is reported back");
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "id,name\nQ153,Ethanol\n"
    );

    let again = download_text_in(&dir, "second", "results.csv").expect("write");
    assert_eq!(again, dir.join("results (2).csv"), "the first is kept");

    let _ = std::fs::remove_dir_all(&dir);
}

/// A name is chosen by the server, and a name containing a path separator
/// must not be able to write outside the download directory.
#[test]
fn a_traversal_in_the_filename_cannot_escape_the_download_directory() {
    let dir = scratch("traversal");

    let path = download_text_in(&dir, "x", "../../escaped.csv").expect("write");
    assert_eq!(
        path.parent().expect("a parent"),
        dir,
        "{path:?} escaped the download directory"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A non-http URL would be handed to an external program, so it is refused.
#[test]
fn only_web_urls_are_handed_to_the_system_opener() {
    for bad in [
        "file:///etc/passwd",
        "javascript:alert(1)",
        "data:text/html,<script>",
        "",
    ] {
        let err = super::open_externally(bad).expect_err("should refuse");
        assert!(err.contains("non-http"), "{bad:?} was not refused: {err}");
    }
}
