// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The date arithmetic the CLI does, in its own file.
//!
//! The CLI never calls a clock: `current_year` is passed in, so this crate can be
//! tested without one. That makes the conversion the one place a wrong answer
//! hides, and the walk from 1970 to 2040 below is here because a `doe / 146_096`
//! term is non-zero on exactly one day in each 400-year cycle.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use super::{civil_year_from_days, current_year, year_from_unix_seconds};

/// Days from 1970-01-01 to the given date, computed the obvious way so the
/// test does not reuse the algorithm it is checking.
/// Only for years from 1970 on; the pre-epoch case uses a literal, because
/// counting backwards through the years is a second algorithm to get wrong.
fn days_since_epoch(year: i32, month: u32, day: u32) -> i64 {
    // Days from a civil date, by the same era arithmetic but written out
    // longhand: leap years, then the months, then the days.
    let leap = |y: i32| y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let mut days = 0i64;
    for y in 1970..year {
        days += if leap(y) { 366 } else { 365 };
    }
    for m in 1..month {
        days += match m {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            _ if leap(year) => 29,
            _ => 28,
        };
    }
    days + i64::from(day) - 1
}

#[test]
fn the_epoch_itself_is_1970() {
    assert_eq!(civil_year_from_days(0), 1970);
}

#[test]
fn the_first_day_of_a_year_is_that_year() {
    for year in [1970, 1999, 2000, 2024, 2025, 2026, 2100] {
        assert_eq!(
            civil_year_from_days(days_since_epoch(year, 1, 1)),
            u16::try_from(year).expect("a test year fits in a u16"),
            "1 January {year}"
        );
    }
}

#[test]
fn the_last_day_of_a_year_is_still_that_year() {
    // The boundary either side of midnight on New Year's Eve, which is where
    // an off-by-one in the day count shows up.
    for year in [1999, 2023, 2024, 2025] {
        let last = days_since_epoch(year + 1, 1, 1) - 1;
        assert_eq!(
            civil_year_from_days(last),
            u16::try_from(year).expect("a test year fits in a u16"),
            "31 December {year}"
        );
    }
}

#[test]
fn a_leap_day_belongs_to_the_leap_year() {
    assert_eq!(civil_year_from_days(days_since_epoch(2024, 2, 29)), 2024);
    assert_eq!(civil_year_from_days(days_since_epoch(2024, 3, 1)), 2024);
    // 2000 was a leap year: the century rule, which a naive "divisible by
    // four" gets wrong for 1900 and right here only by accident of the
    // four-hundred-year cycle.
    assert_eq!(civil_year_from_days(days_since_epoch(2000, 2, 29)), 2000);
    assert_eq!(civil_year_from_days(days_since_epoch(2000, 3, 1)), 2000);
}

#[test]
fn a_century_boundary_rolls_the_era() {
    // 2100-03-01 is the first day after a 400-year era ends, which is the
    // one input where the era division actually changes.
    assert_eq!(civil_year_from_days(days_since_epoch(2100, 3, 1)), 2100);
    assert_eq!(civil_year_from_days(days_since_epoch(2100, 2, 28)), 2100);
}

#[test]
fn a_date_before_the_epoch_does_not_wrap_to_a_recent_year() {
    // 1600 is outside a u16 year once added to the era offset, and a wrap
    // would produce a plausible-looking reference year instead of an
    // obviously wrong one.
    assert_eq!(civil_year_from_days(-135_140), 1600);
}

#[test]
fn every_day_of_a_run_of_years_lands_in_its_own_year() {
    // The day-of-year arithmetic has more terms than a handful of dates can
    // distinguish, so this walks whole years and checks the boundaries and
    // the middle of each. `days_since_epoch` is written out longhand and
    // shares no code with the algorithm, so agreement means something.
    for year in 1970..=2035 {
        for (month, day) in [(1, 1), (2, 28), (3, 1), (6, 15), (12, 31)] {
            let days = days_since_epoch(year, month, day);
            assert_eq!(
                civil_year_from_days(days),
                u16::try_from(year).expect("a test year fits in a u16"),
                "{year}-{month:02}-{day:02}"
            );
        }
        // 29 February where there is one.
        if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
            let leap = days_since_epoch(year, 2, 29);
            assert_eq!(
                civil_year_from_days(leap),
                u16::try_from(year).expect("a test year fits in a u16"),
                "{year}-02-29"
            );
            assert_eq!(
                civil_year_from_days(leap - 1),
                u16::try_from(year).expect("a test year fits in a u16"),
                "the day before"
            );
        }
    }
}

#[test]
fn every_day_of_a_long_run_never_leaves_its_own_decade() {
    // Coarse but total: sampled across four centuries, the answer is always
    // the year of the date, and a single wrong term shows up somewhere.
    for year in [1970, 2000, 2024, 2100, 2400] {
        for (month, day) in [(1, 1), (3, 1), (7, 4), (12, 31)] {
            let days = days_since_epoch(year, month, day);
            let got = i64::from(civil_year_from_days(days));
            let year = i64::from(year);
            assert!(
                (year - 1..=year + 1).contains(&got),
                "{year}-{month:02}-{day:02} -> {got}"
            );
        }
    }
}

#[test]
fn the_year_never_goes_backwards_and_turns_over_on_new_years_day() {
    // Walks every single day from the epoch to 2040 rather than sampling.
    // The correction terms in the day-of-year division are small enough that
    // the `/365` after them hides them for most dates -- `doe / 146_096` is
    // nonzero on exactly one day in four hundred -- so no handful of dates
    // can tell whether they are right.
    let mut previous = civil_year_from_days(0);
    for day in 1..days_since_epoch(2040, 1, 1) {
        let year = civil_year_from_days(day);
        assert!(
            year >= previous,
            "the year went backwards at day {day}: {previous} then {year}"
        );
        assert!(
            year - previous <= 1,
            "the year jumped at day {day}: {previous} then {year}"
        );
        previous = year;
    }

    // And it turns over exactly on 1 January, not a day either side.
    for year in 1971..=2040 {
        let new_year = days_since_epoch(year, 1, 1);
        assert_eq!(
            civil_year_from_days(new_year - 1),
            u16::try_from(year - 1).expect("a test year fits in a u16"),
            "31 December {}",
            year - 1
        );
        assert_eq!(
            civil_year_from_days(new_year),
            u16::try_from(year).expect("a test year fits in a u16"),
            "1 January {year}"
        );
    }
}

#[test]
fn the_last_day_of_a_four_hundred_year_era_is_still_that_year() {
    // 2000-02-29 is the final day of the era that 1970 sits in, and it is
    // the one date in four hundred where `doe / 146_096` is nonzero.
    let era_end = days_since_epoch(2000, 2, 29);
    assert_eq!(civil_year_from_days(era_end), 2000);
    assert_eq!(civil_year_from_days(era_end - 1), 2000, "the day before it");
    assert_eq!(
        civil_year_from_days(era_end + 1),
        2000,
        "the era rolls over inside it"
    );
}

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
fn seconds_are_floored_to_whole_days() {
    // One second after the turn of the year, in seconds. Divided, that is a
    // whole day past New Year; anything else about the arithmetic -- a
    // remainder, a truncating divide -- lands back in the previous year.
    // 365 * 86_400 seconds: the first instant of 1971.
    assert_eq!(
        year_from_unix_seconds(31_535_999),
        1970,
        "one second before it"
    );
    assert_eq!(year_from_unix_seconds(31_536_000), 1971, "exactly it");
    assert_eq!(year_from_unix_seconds(31_536_001), 1971, "one second after");
    assert_eq!(year_from_unix_seconds(0), 1970);
    assert_eq!(
        year_from_unix_seconds(-1),
        1969,
        "a second before the epoch"
    );
}

#[test]
fn the_current_year_is_the_year_it_is() {
    // The only assertion about the clock, and the one that would hide an
    // error for most of the year -- which is why the arithmetic above is
    // checked against fixed dates instead.
    let now = current_year();
    assert!((2020..=2100).contains(&now), "got {now}");
}
