// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Which failures are worth retrying, and which are final.
//!
//! `types.rs` had no test module at all, and it holds the decision a curation
//! run makes about every row it cannot finish: retry, or stop. Two rows failing
//! for the same reason must get the same answer, and the reasoning is only in the
//! doc comments above the two functions.

#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use super::{CurationError, CurationErrorKind};

/// Every variant, with a value that is plausible for it.
fn every_error() -> [(CurationError, CurationErrorKind, bool); 4] {
    [
        (
            CurationError::InvalidInput("not a SMILES".into()),
            CurationErrorKind::InvalidInput,
            false,
        ),
        (
            CurationError::MissingTsvColumn("smiles"),
            CurationErrorKind::InvalidInput,
            false,
        ),
        (
            CurationError::Http("connection refused".into()),
            CurationErrorKind::Transport,
            true,
        ),
        (
            CurationError::Parse("no result rows".into()),
            CurationErrorKind::Parse,
            false,
        ),
    ]
}

#[test]
fn a_transport_failure_is_the_only_one_worth_retrying() {
    // Only because the input was never in question. A parse failure means the
    // answer is there and is not in a shape this crate reads, so a second
    // request is refused the same way; an input failure means the same malformed
    // structure is the same malformed structure. Both just spend the endpoint's
    // rate limit.
    for (error, _, recoverable) in every_error() {
        assert_eq!(
            error.is_recoverable(),
            recoverable,
            "{error:?}: the retry decision is what a run uses to decide whether to \\
             spend the endpoint's budget again"
        );
    }
}

#[test]
fn both_input_failures_share_one_kind() {
    // Two variants, one kind: a missing column and an unusable value are the
    // same problem to whoever has to fix the file, and a caller switching on
    // `kind` to decide whether to ask them to fix something must get one answer
    // rather than two that happen to sit next to each other.
    assert_eq!(
        CurationError::InvalidInput("x".into()).kind(),
        CurationError::MissingTsvColumn("smiles").kind(),
        "a missing column and an unusable value are one kind of problem"
    );
    assert_eq!(
        CurationError::InvalidInput("x".into()).kind(),
        CurationErrorKind::InvalidInput
    );
}

#[test]
fn the_kind_is_what_the_retry_decision_follows() {
    // The two functions are related but not the same thing, and collapsing them
    // would be wrong in one direction: a `Parse` kind is not recoverable, and an
    // `InvalidInput` kind is not either. Asserted together so the pairing is
    // visible rather than implied by two separate tests.
    for (error, kind, _) in every_error() {
        assert_eq!(error.kind(), kind, "{error:?} is classified wrongly");
        assert_eq!(
            error.is_recoverable(),
            matches!(kind, CurationErrorKind::Transport),
            "{error:?}: recoverability and kind must agree, or a caller switching \\
             on one gets a different answer from a caller switching on the other"
        );
    }
}

#[test]
fn a_missing_column_names_the_column_in_the_message() {
    // The user has to add the column, so the message has to say which one. A
    // generic "missing column" sends them back to the header to work it out.
    let error = CurationError::MissingTsvColumn("taxon");
    assert!(
        error.to_string().contains("'taxon'"),
        "the message must name the column: {error}"
    );
}
