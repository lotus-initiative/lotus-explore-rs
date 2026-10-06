// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The three shapes of `--carbon`, `--nitrogen` and friends.
//!
//! The bare number and the empty-lower-bound form both mean "at most N", and
//! neither had a test. They are the two a chemist types most, and both reach a
//! formula filter that turns a search into something much narrower -- so getting
//! one silently wrong narrows a search without saying so.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use super::Range;

/// Parse `value` the way clap does for `--carbon`.
fn parse(value: &str) -> Result<Range, String> {
    value.parse()
}

#[test]
fn a_bare_number_is_an_upper_bound() {
    // `--carbon 5` reads as "at most 5 carbons", which is the bound a chemist
    // narrows with. A lower bound alone selects almost nothing, so `20..` is how
    // you ask for that -- which is why the bare form is the upper one and not
    // the lower one.
    for value in ["5", "0", "30"] {
        let range = parse(value).expect("a bare number parses");
        assert_eq!(
            (range.min, range.max),
            (0, value.parse::<u16>().expect("the fixture is a number")),
            "--carbon {value} means at most {value}, not at least"
        );
    }
}

#[test]
fn an_explicit_empty_lower_bound_means_the_same_as_the_bare_form() {
    // `..5` is "at most 5" with the lower end written out, and it agrees with
    // the bare form. That arm is correct; its sibling is not -- see
    // `a_lower_bound_only_spelling_is_still_an_upper_bound_today`.
    assert_eq!(
        parse("..5").expect("parses").max,
        parse("5").expect("parses").max
    );
    assert_eq!(parse("..5").expect("parses").min, 0);
}

/// **Documents a defect rather than the intended behaviour. Read this before
/// "fixing" the test.**
///
/// The two range arms set the same field:
///
/// ```text
/// Some((min, "")) => Ok(Self { min: 0, max: min.parse()? }),   // "20.." -> 0..20
/// Some(("", max)) => Ok(Self { min: 0, max: max.parse()? }),   // "..5"  -> 0..5
/// ```
///
/// so `20..` and `..20` are the same input meaning "at most 20", and there is no
/// way to ask for a lower bound alone. The comment eight lines below the match
/// says the opposite -- "A lower bound alone selects almost nothing, so `20..`
/// is how you ask for that" -- and the CLI's own help advertises `MIN..`.
///
/// A user typing `--carbon 20..` expecting "at least 20 carbons" silently gets
/// "at most 20", which is a much narrower search pointing the wrong way, and
/// nothing in the output says so.
///
/// Not fixed here: this branch may not change production code, and this test
/// asserts what the code does rather than what it should. The assertion below is
/// deliberately written as `0..20`, not `20..MAX`, so that fixing the code makes
/// this test FAIL and forces whoever does it to read this comment.
#[test]
fn a_lower_bound_only_spelling_is_still_an_upper_bound_today() {
    let bare_upper = parse("20").expect("parses");
    let trailing_dots = parse("20..").expect("parses");
    let leading_dots = parse("..20").expect("parses");

    assert_eq!(
        (trailing_dots.min, trailing_dots.max),
        (0, 20),
        "`20..` is documented as the lower-bound spelling and currently parses as \
         an upper bound. When this is fixed this assertion is what must change."
    );
    assert_eq!(
        (trailing_dots.min, trailing_dots.max),
        (leading_dots.min, leading_dots.max),
        "`20..` and `..20` are the same parse; the two spellings of one end are \
         indistinguishable, which is the shape of the defect"
    );
    assert_eq!(
        (bare_upper.min, bare_upper.max),
        (trailing_dots.min, trailing_dots.max),
        "and so is a bare number, which is correct for a bare number"
    );
}

#[test]
fn a_two_sided_range_is_the_only_way_to_get_a_lower_bound() {
    let (upper, both) = (
        parse("5").expect("parses"),
        parse("10..20").expect("parses"),
    );
    assert_eq!((upper.min, upper.max), (0, 5), "a bare number is a ceiling");
    assert_eq!(
        (both.min, both.max),
        (10, 20),
        "and `10..20` is the one form that has both"
    );

    // `..` with neither end is not a filter, and saying so is better than
    // returning the full range and letting the caller wonder.
    let error = parse("..").expect_err("an empty range is not a filter");
    assert!(
        error.contains("MIN..MAX"),
        "the message must say what was expected: {error:?}"
    );
}

#[test]
fn a_malformed_bound_is_refused_with_the_whole_set_of_spellings() {
    for value in ["abc", "5..abc", "abc..5", "-1..5", "5..20..30"] {
        let error = parse(value).unwrap_err();
        assert!(
            error.contains("MIN..MAX") && error.contains("MIN..") && error.contains("..MAX"),
            "the message for {value:?} must list every accepted spelling: {error:?}"
        );
    }
}
