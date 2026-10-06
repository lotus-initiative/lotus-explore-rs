// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The truncation notice, against responses captured from `qlever.dev`.
//!
//! These are whole real bodies, not fixtures: the whole point of the notice is
//! that it is what separates a complete answer from a cut one, and a
//! hand-written fixture would only prove the parser agrees with itself.

#![allow(unused_crate_dependencies)]
// A test reporting a bad capture is reporting, not panicking on library input.
#![allow(clippy::expect_used)]

/// A response that ended with the notice, and one that did not.
const CAPTURED: &[(&str, &str, bool)] = &[];

#[test]
fn captured_responses_are_read_from_the_environment() {
    // The bodies are 194-326 MB each and are not checked in. Point this at a
    // directory of captures to re-verify; with none set, there is nothing to do
    // and that is not a failure.
    let Ok(dir) = std::env::var("LOTUS_TRUNCATION_CAPTURES") else {
        assert!(CAPTURED.is_empty(), "no captures configured");
        return;
    };
    for entry in std::fs::read_dir(dir).expect("capture directory is readable") {
        let path = entry.expect("readable entry").path();
        let bytes = std::fs::read(&path).expect("readable capture");
        let marked = bytes
            .windows(b"!!!!>>#".len())
            .any(|window| window == b"!!!!>>#");
        match lotus_query::parse_compounds_columnar(bytes.as_slice()) {
            Ok(set) => assert!(
                !marked,
                "{} parsed to {} rows but carries the notice",
                path.display(),
                set.row_count()
            ),
            Err(error) => assert!(
                marked,
                "{} was refused without carrying the notice: {error}",
                path.display()
            ),
        }
    }
}
