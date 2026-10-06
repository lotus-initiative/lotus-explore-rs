// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `stats`, in their own file.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use super::{ElementState, SmilesSearchType};

// The spellings are the wire format: a form control's value comes back in a
// URL, and the URL is something a person can edit. Each variant therefore
// has exactly one string, and a change to it is a change to every saved
// search.

#[test]
fn each_search_type_prints_exactly_its_own_spelling() {
    assert_eq!(SmilesSearchType::Substructure.to_string(), "substructure");
    assert_eq!(SmilesSearchType::Similarity.to_string(), "similarity");
}

#[test]
fn each_element_state_prints_exactly_its_own_spelling() {
    assert_eq!(ElementState::Allowed.to_string(), "allowed");
    assert_eq!(ElementState::Required.to_string(), "required");
    assert_eq!(ElementState::Excluded.to_string(), "excluded");
}

#[test]
fn parsing_is_case_and_whitespace_insensitive() {
    for (text, expected) in [
        ("substructure", SmilesSearchType::Substructure),
        ("  SUBSTRUCTURE  ", SmilesSearchType::Substructure),
        ("Similarity", SmilesSearchType::Similarity),
    ] {
        assert_eq!(SmilesSearchType::parse(text), expected, "{text:?}");
    }
    for (text, expected) in [
        ("allowed", ElementState::Allowed),
        (" REQUIRED ", ElementState::Required),
        ("Excluded", ElementState::Excluded),
    ] {
        assert_eq!(
            text.parse::<ElementState>().unwrap_or_default(),
            expected,
            "{text:?}"
        );
    }
}

#[test]
fn printing_then_parsing_returns_the_same_value() {
    // A round trip is what keeps a shared link working after a rename.
    for search in [SmilesSearchType::Substructure, SmilesSearchType::Similarity] {
        assert_eq!(SmilesSearchType::parse(&search.to_string()), search);
    }
    for state in [
        ElementState::Allowed,
        ElementState::Required,
        ElementState::Excluded,
    ] {
        assert_eq!(
            state
                .to_string()
                .parse::<ElementState>()
                .unwrap_or_default(),
            state
        );
    }
}

#[test]
fn an_unrecognised_spelling_falls_back_rather_than_failing() {
    // Note the two ways in: `ElementState` is reached through `FromStr`,
    // `SmilesSearchType` through an inherent `parse`. Both are infallible, so
    // `parse` cannot fail and the tests use `unwrap_or_default` only to keep
    // the comparison types lined up.
    assert_eq!(
        "maybe".parse::<ElementState>().unwrap_or_default(),
        ElementState::Allowed,
        "the default is the absence of a constraint"
    );
    assert_eq!(SmilesSearchType::parse("fuzzy"), SmilesSearchType::Exact);
}
