// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `reference_metadata`, in their own file.

#![allow(clippy::expect_used)]

use super::parse_quickstatements_text;

#[test]
fn parse_quickstatements_text_filters_blank_lines() {
    let parsed = parse_quickstatements_text("\nCREATE\n\nLAST|P31|Q123\n  \n")
        .expect("parsed quickstatements");
    assert_eq!(parsed, vec!["CREATE", "LAST|P31|Q123"]);
}

#[test]
fn parse_quickstatements_text_returns_none_for_empty_input() {
    assert_eq!(parse_quickstatements_text("  \n\n\t"), None);
}
