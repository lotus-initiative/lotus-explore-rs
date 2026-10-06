// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `clock`, in their own file.

use super::*;

#[test]
fn the_year_is_this_century() {
    // A wide band: the point is to catch an arithmetic slip, not to fail in
    // 2099. On a host with no clock this is the fallback, which is also
    // checked below.
    let year = current_year();
    assert!((2020..=2100).contains(&year), "{year}");
}

#[test]
fn known_days_map_to_known_years() {
    // Pinned against published values rather than the implementation, so
    // this fails if the arithmetic changes rather than if it changes the
    // same way twice.
    for (days, year) in [
        (0_i64, 1_970_u16), // the epoch itself
        (11_016, 2_000),    // 1 Jan 2000
        (19_723, 2_024),    // 1 Jan 2024, a leap year
        (-2_556, 1_963),    // before the epoch
        (45_999, 2_095),    // a century on
    ] {
        assert_eq!(year_from_days_since_epoch(days), year, "{days} days");
    }
}

#[test]
fn a_leap_day_belongs_to_its_own_year() {
    // 29 Feb 2024 is 19_782 days after the epoch and 1 Mar is 19_783.
    // Getting this wrong is a one-day error, which is invisible at a
    // December-to-January boundary and wrong at a February one.
    assert_eq!(year_from_days_since_epoch(19_782), 2_024);
    assert_eq!(year_from_days_since_epoch(19_783), 2_024);
    assert_eq!(year_from_days_since_epoch(20_089), 2_025);
}

#[test]
fn a_century_boundary_is_not_a_leap_year() {
    // 1900 was not a leap year and 2000 was. The algorithm has to get both
    // right or the day count drifts by one every 100 years.
    assert_eq!(year_from_days_since_epoch(-25_567), 1_900);
    assert_eq!(year_from_days_since_epoch(10_957), 2_000);
}
