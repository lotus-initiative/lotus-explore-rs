// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `reqwest_client`, in their own file.

use super::*;

#[test]
fn a_read_that_failed_before_any_byte_is_still_a_connection_problem() {
    // Nothing was transferred, so sending the request again costs nothing
    // and might work. This is the retryable half.
    let err = classify_read_failure(0, "connection reset".into());
    assert!(matches!(err, FetchError::Network(_)), "{err:?}");
    assert!(err.is_retryable());
}

#[test]
fn a_read_that_failed_after_bytes_is_a_cut_short_result_set() {
    // This is the rule the timeout complaint actually turns on: the same
    // socket error, but with half a gigabyte already transferred, and so
    // not something to answer by starting over.
    let err = classify_read_failure(4096, "connection reset".into());
    assert!(err.is_truncated(), "{err:?}");
    assert!(!err.is_retryable());
}

#[test]
fn the_one_byte_threshold_is_a_real_boundary_and_not_a_range() {
    // Mutating `> 0` to `>= 0` would make every failure untouchable if it
    // were wrong in the other direction, and `>= 1` to `> 0` would make every
    // failure retryable. Both are single-character changes with opposite
    // effects, so both ends are pinned.
    assert!(classify_read_failure(0, "x".into()).is_retryable());
    assert!(classify_read_failure(1, "x".into()).is_truncated());
}
