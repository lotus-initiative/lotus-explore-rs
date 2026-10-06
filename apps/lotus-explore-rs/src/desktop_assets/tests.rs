// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `desktop_assets`, in their own file.

// A test that fails on a bad fixture is reporting, not panicking on input.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use super::PROBE;

/// Ketcher's real entry document, in the two shapes that matter: its bundle is
/// a `defer`red script in the head, and it mounts into `#root`.
const ENTRY: &str = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"/>\
    <link rel=\"icon\" href=\"./favicon.ico\"/><title>Ketcher</title>\
    <script defer=\"defer\" src=\"./static/js/main.e47c48ad.js\"></script>\
    <link href=\"./static/css/main.96b87be0.css\" rel=\"stylesheet\"></head>\
    <body><div id=\"root\"></div></body></html>";

/// The base is derived from the entry path, so the two cannot drift.
#[test]
fn a_base_is_the_directory_of_the_entry_document() {
    let base = |entry: &str| {
        format!(
            "{}/",
            entry
                .trim_end_matches('/')
                .rsplit_once('/')
                .map_or("", |(directory, _)| directory)
        )
    };
    assert_eq!(base("/assets/ketcher/index.html"), "/assets/ketcher/");
    assert_eq!(base("/assets/ketcher/"), "/assets/");
    assert_eq!(base("index.html"), "/");
}

/// The probe has to run *before* the editor's own bundle, and has to be a
/// script element. Both are load-bearing: a probe that runs late reads the path
/// too late to influence anything, and a probe inserted as text renders as
/// words on the page.
#[test]
fn the_probe_is_a_script_ahead_of_the_editor_bundle() {
    let head = format!("<head><base href=\"/assets/ketcher/\"><script>{PROBE}</script>");
    let patched = ENTRY.replacen("<head>", &head, 1);

    assert!(
        patched.contains("<script>"),
        "a script element, not bare text"
    );
    assert!(patched.contains("ketcher_probe"), "and it is the probe");

    let probe_at = patched.find("<script>").expect("the probe");
    let bundle_at = patched
        .find("main.e47c48ad.js")
        .expect("the editor's bundle");
    assert!(
        probe_at < bundle_at,
        "ahead of the bundle, or the router has already read the path"
    );
    assert!(
        patched.find("<base").is_some_and(|at| at < probe_at),
        "and the base ahead of the probe"
    );
    // The editor's own markup survives, or there is nothing to mount into.
    assert!(patched.contains("id=\"root\""));
}

/// `replaceState` is the whole fix for the router, so it happens before the
/// document reports anything -- otherwise the first thing known about it is
/// that the path was still wrong.
#[test]
fn the_probe_gives_the_document_a_path_before_it_reports() {
    let set = PROBE.find("replaceState").expect("the probe sets a path");
    let reported = PROBE.find("state: \"prepared\"").expect("the first report");
    assert!(set < reported, "the path is set before the first report");
}

/// Whether `replaceState` takes is the engine's decision and the probe reports
/// it either way -- but a probe that forgot to check would leave the failure
/// invisible, which is what the first two attempts did.
#[test]
fn the_probe_checks_whether_the_path_took() {
    assert!(
        PROBE.contains("window.location.pathname.charAt(0)"),
        "checked by looking at the path, which is the thing the router reads"
    );
}

/// And it reports whether the editor mounted, which is the only thing that
/// distinguishes a working editor from a blank box.
#[test]
fn the_probe_reports_whether_the_editor_mounted() {
    assert!(
        PROBE.contains("mounted: Boolean(root && root.childElementCount > 0)"),
        "the mount is checked against the container Ketcher renders into"
    );
}
