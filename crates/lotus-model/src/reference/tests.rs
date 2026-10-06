// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `reference`, in their own file.

use super::*;

#[test]
fn a_qid_is_recognised_in_either_case() {
    assert!(looks_like_reference_qid("Q18216"));
    assert!(looks_like_reference_qid("q18216"));
    assert!(looks_like_reference_qid("  Q18216  "));
    assert!(looks_like_reference_qid("Q1"));
}

#[test]
fn something_that_is_not_a_qid_is_not_one() {
    // `Q` alone is a prefix a reader has not finished typing, and the rest
    // are not identifiers.
    for text in [
        "",
        "   ",
        "Q",
        "q",
        "Q18a16",
        "CCQ",
        "quinine",
        "10.1000/xyz",
    ] {
        assert!(!looks_like_reference_qid(text), "{text} is not a QID");
    }
}

#[test]
fn a_doi_is_recognised() {
    assert!(looks_like_doi("10.1000/xyz"));
    assert!(looks_like_doi("10.1002/andp.18280880206"));
    assert!(looks_like_doi("10.1000/xyz789"));
}

#[test]
fn a_doi_arrives_with_any_of_the_prefixes_a_reader_pastes() {
    // All four of these turn up in a paper, an email and a reference manager.
    for text in [
        "10.1000/xyz",
        "doi:10.1000/xyz",
        "https://doi.org/10.1000/xyz",
        "http://dx.doi.org/10.1000/xyz",
    ] {
        assert!(looks_like_doi(text), "{text} should be a DOI");
        assert_eq!(strip_doi_prefix(text), "10.1000/xyz");
    }
}

#[test]
fn a_doi_suffix_may_be_upper_case() {
    // DOIs are case-insensitive and are routinely written that way in prose.
    assert!(looks_like_doi("10.1000/XYZ"));
    assert_eq!(strip_doi_prefix("10.1000/XYZ"), "10.1000/XYZ");
}

#[test]
fn something_that_is_not_a_doi_is_not_one() {
    for text in [
        "",
        "   ",
        "10.",
        "10.10/x",     // two-digit registrant prefix
        "11.1000/xyz", // not the DOI prefix
        "1000/xyz",    // no `10.`
        "10.1000/",    // nothing after the slash
        "10.1000/  ",  // blank suffix
        "10.1000/a b", // whitespace inside
        "abc/def",
        "Gentiana lutea",
    ] {
        assert!(!looks_like_doi(text), "{text} should not be a DOI");
    }
}

#[test]
fn a_qid_is_never_mistaken_for_a_doi_and_the_other_way_round() {
    // The two routes have to be disjoint, or a QID would cost a pointless
    // round trip asking Wikidata to look up a DOI it already holds.
    assert!(!looks_like_doi("Q18216"));
    assert!(!looks_like_reference_qid("10.1000/xyz"));
}

#[test]
fn a_non_ascii_reference_field_is_an_answer_rather_than_a_panic() {
    // Every prefix length is a byte count, so an input whose `prefix.len()`-th
    // byte lands inside a character makes a range slice split that character.
    // These two straddle the boundary: one where the split falls inside the
    // longest prefix, one inside the shortest.
    let straddles_longest = format!("{}\u{3b1}rest", "A".repeat(15));
    let straddles_shortest = format!("{}\u{3b1}rest", "A".repeat(4));
    for text in [&straddles_longest, &straddles_shortest] {
        assert!(!looks_like_doi(text), "{text:?} is not a DOI");
        assert_eq!(strip_doi_prefix(text), text);
        assert!(!looks_like_reference_qid(text));
    }
    // The same shape with a prefix already stripped: a DOI is not required to
    // be ASCII, and the suffix is only checked for emptiness and whitespace,
    // so this has to reach a verdict rather than panic on the way to one.
    assert!(!looks_like_doi("\u{3b1}10.1000/xyz"));
    assert!(looks_like_doi("10.1000/xyz\u{2014}"));
}
