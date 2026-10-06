// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `upload`, in their own file.

// The panic lints keep shipped code free of panics on external input. A test
// that fails to create its temp dir is reporting, not panicking.

use super::sanitize_filename;

#[test]
fn sanitize_removes_path_separators() {
    assert_eq!(sanitize_filename("a/b\\c"), "a_b_c");
}

#[test]
fn sanitize_strips_control_chars() {
    assert_eq!(sanitize_filename("file\x00name"), "filename");
}

#[test]
fn sanitize_strips_leading_dots() {
    assert_eq!(sanitize_filename("...file.txt"), "file.txt");
}

#[test]
fn sanitize_empty_input() {
    assert_eq!(sanitize_filename("   "), "");
    assert_eq!(sanitize_filename("."), "");
}

#[test]
fn sanitize_replaces_quotes_with_underscore() {
    assert_eq!(sanitize_filename("file\"name"), "file_name");
    assert_eq!(sanitize_filename("file'name"), "file_name");
}

#[test]
fn sanitize_preserves_safe_names() {
    assert_eq!(sanitize_filename("lotus_results.csv"), "lotus_results.csv");
    assert_eq!(sanitize_filename("my_file-01.json"), "my_file-01.json");
}

#[test]
fn sanitize_strips_trailing_whitespace() {
    assert_eq!(sanitize_filename("file.txt "), "file.txt");
    assert_eq!(sanitize_filename(" file.txt"), "file.txt");
}

#[test]
fn sanitize_unicode_passthrough() {
    assert_eq!(sanitize_filename("résultats.csv"), "résultats.csv");
    assert_eq!(sanitize_filename("α-β-γ.rdf"), "α-β-γ.rdf");
}
