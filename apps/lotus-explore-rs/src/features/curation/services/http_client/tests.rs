// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `http_client`, in their own file.

#![allow(clippy::expect_used)]

// A test that fails to encode is reporting, not panicking.

/// The SMILES is user input and goes into a JS string literal. The property
/// that matters is that the literal parses back to exactly the input, so
/// nothing inside it can terminate the literal early and change what the
/// script runs.
#[test]
fn a_structure_round_trips_through_its_string_literal() {
    for input in [
        "CCO",
        r#"C/C=C/C""#,      // a double quote
        r"C\\C",            // a backslash
        "'); alert(1); ('", // an attempt to close the literal
        "C[C@@H](C(=O)O)N", // stereochemistry
        "line\\nbreak",     // a newline
    ] {
        let literal = serde_json::to_string(input).expect("encodes");
        let back: String = serde_json::from_str(&literal).expect("the literal parses back");
        assert_eq!(back, input, "{literal} did not round-trip");
        assert!(literal.starts_with('"'), "{literal} is not double-quoted");
    }
}
