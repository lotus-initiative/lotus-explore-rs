// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The clock and flag helpers of the CLI, in their own file.
//!
//! The calendar arithmetic is `chrono`'s: a hand-written civil-date conversion
//! used to live here and got 1 March 1970 wrong, which is what these tests no
//! longer need to guard.

#![allow(clippy::expect_used)]

use super::{current_year, iso_utc, now_iso_utc};

/// A criteria with nothing asked of it.
fn plain() -> lotus_model::SearchCriteria {
    lotus_model::SearchCriteria::up_to_year(2026)
}

#[test]
fn each_formula_flag_asks_for_formula_filtering_on_its_own() {
    // `--carbon 10..20` has filtered by formula whether or not `--formula`
    // was also given, so each of these has to turn it on by itself. The
    // chain is a row of `||`, and a test that set two flags at once would
    // pass with any single one of them removed.
    assert!(!super::flags_ask_for_formula(&plain()), "nothing asked");

    let mut exact = plain();
    exact.formula_exact = "C6H6O".into();
    assert!(super::flags_ask_for_formula(&exact), "--formula");

    let mut blank = plain();
    blank.formula_exact = "   ".into();
    assert!(
        !super::flags_ask_for_formula(&blank),
        "a blank --formula is nothing"
    );

    let mut lower = plain();
    lower.c_min = 5;
    assert!(
        super::flags_ask_for_formula(&lower),
        "--carbon with a lower bound"
    );

    let mut upper = plain();
    upper.c_max = lotus_model::element_max::C - 1;
    assert!(
        super::flags_ask_for_formula(&upper),
        "--carbon with an upper bound"
    );

    let mut untouched = plain();
    untouched.c_min = 0;
    untouched.c_max = lotus_model::element_max::C;
    assert!(
        !super::flags_ask_for_formula(&untouched),
        "the full carbon range is not a filter"
    );
}

#[test]
fn each_halogen_on_its_own_asks_for_formula_filtering() {
    use lotus_model::ElementState;
    for state in [ElementState::Required, ElementState::Excluded] {
        // One halogen at a time: the four are a row of `||` comparing
        // against `Allowed`, and setting two would hide a broken third.
        for which in ["f", "cl", "br", "i"] {
            let mut criteria = plain();
            match which {
                "f" => criteria.f_state = state,
                "cl" => criteria.cl_state = state,
                "br" => criteria.br_state = state,
                _ => criteria.i_state = state,
            }
            assert!(
                super::flags_ask_for_formula(&criteria),
                "{state:?} on {which} is a filter"
            );
        }
    }
    // Allowed on all four is the absence of a constraint, which is what the
    // other three comparisons are made against.
    let mut allowed = plain();
    for state in [ElementState::Allowed; 4] {
        allowed.f_state = state;
        allowed.cl_state = state;
        allowed.br_state = state;
        allowed.i_state = state;
    }
    assert!(!super::flags_ask_for_formula(&allowed));
}

#[test]
fn the_current_year_is_the_year_it_is() {
    // The only assertion about the clock: the formatting is checked against
    // fixed instants below.
    let now = current_year();
    assert!((2020..=2100).contains(&now), "got {now}");
}

#[test]
fn an_instant_is_stamped_to_the_second() {
    // Whole seconds and a literal `Z`: `to_rfc3339` would write `+00:00`, which
    // the metadata documents do not carry.
    for (seconds, expected) in [
        (0, "1970-01-01T00:00:00Z"),
        (45_296, "1970-01-01T12:34:56Z"),
        (86_399, "1970-01-01T23:59:59Z"),
        (1_700_000_000, "2023-11-14T22:13:20Z"),
        (-1, "1969-12-31T23:59:59Z"),
    ] {
        let instant = chrono::DateTime::from_timestamp(seconds, 0).expect("in range");
        assert_eq!(iso_utc(instant), expected, "{seconds}s");
    }
}

#[test]
fn the_clock_stamp_is_a_parseable_recent_instant() {
    let now = now_iso_utc();
    assert_eq!(now.len(), "1970-01-01T00:00:00Z".len(), "got {now:?}");
    let parsed = super::parse_iso_utc(&now).expect("now_iso_utc parses as RFC 3339");
    let year = u16::try_from(chrono::Datelike::year(&parsed)).expect("a recent year fits");
    assert!((2020..=2100).contains(&year), "got {now:?}");
}
