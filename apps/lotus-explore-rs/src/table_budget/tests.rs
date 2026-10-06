// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `table_budget`, in their own file.

use super::*;

#[test]
fn the_export_ceiling_is_not_a_rendering_limit() {
    // A `const` block, because both operands are constants and the assertion
    // is about the relationship between them rather than about a runtime
    // value. The message is a static string: a const block cannot format.
    const _: () = assert!(
        API_MAX_ROWS > 0,
        "a limit of zero means the caller can never have a result"
    );
}

#[test]
fn the_export_ceiling_is_large_enough_to_be_usable() {
    // The whole graph is about three million rows and the widest interactive
    // search is bounded by whatever the endpoint will answer, not by this. A
    // bulk caller asking for a genus should not be told "no" by a number that
    // was once tuned to a screen.
    //
    // A `const` block, as above: both operands are constants, so this asserts
    // a relationship at compile time rather than re-checking a literal.
    const _: () = assert!(
        API_MAX_ROWS >= 1_000_000,
        "the export ceiling has collapsed below what a bulk caller needs"
    );
}
