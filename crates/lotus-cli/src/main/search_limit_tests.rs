// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! When `search` warns that the `--limit` may have hidden rows.
//!
//! The rows come from the network, so the decision is checked here, on its own.

use super::hit_limit;

#[test]
fn no_limit_never_warns() {
    assert!(!hit_limit(None, true, 10));
    assert!(!hit_limit(None, false, 10));
}

#[test]
fn a_truncated_result_warns_even_below_the_limit() {
    assert!(hit_limit(Some(5), true, 0));
}

#[test]
fn reaching_the_limit_warns_and_falling_short_does_not() {
    assert!(!hit_limit(Some(5), false, 4));
    assert!(hit_limit(Some(5), false, 5));
    assert!(hit_limit(Some(5), false, 6));
}
