// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Which failures are worth retrying, and which are final.

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
    for (error, _, recoverable) in every_error() {
        assert_eq!(
            error.is_recoverable(),
            recoverable,
            "{error:?}: the retry decision is what a run uses to decide whether to \
             spend the endpoint's budget again"
        );
    }
}

#[test]
fn both_input_failures_share_one_kind() {
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
    for (error, kind, _) in every_error() {
        assert_eq!(error.kind(), kind, "{error:?} is classified wrongly");
        assert_eq!(
            error.is_recoverable(),
            matches!(kind, CurationErrorKind::Transport),
            "{error:?}: recoverability and kind must agree, or a caller switching \
             on one gets a different answer from a caller switching on the other"
        );
    }
}

#[test]
fn a_missing_column_names_the_column_in_the_message() {
    let error = CurationError::MissingTsvColumn("taxon");
    assert!(
        error.to_string().contains("'taxon'"),
        "the message must name the column: {error}"
    );
}
