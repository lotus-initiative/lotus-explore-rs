// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Unit tests for pure UTC calendar arithmetic and RFC 3339 parsing.

#![allow(clippy::expect_used)]

use super::*;

#[test]
fn unix_seconds_format_exact_iso_and_compact_stamps() {
    let cases = [
        (
            0_i64,
            "1970-01-01T00:00:00Z",
            "19700101T000000Z",
            "1970-01-01",
        ),
        (
            5_097_600,
            "1970-03-01T00:00:00Z",
            "19700301T000000Z",
            "1970-03-01",
        ),
        (
            45_296,
            "1970-01-01T12:34:56Z",
            "19700101T123456Z",
            "1970-01-01",
        ),
        (
            86_399,
            "1970-01-01T23:59:59Z",
            "19700101T235959Z",
            "1970-01-01",
        ),
        (
            1_700_000_000,
            "2023-11-14T22:13:20Z",
            "20231114T221320Z",
            "2023-11-14",
        ),
        (-1, "1969-12-31T23:59:59Z", "19691231T235959Z", "1969-12-31"),
    ];
    for (secs, iso, compact, date) in cases {
        let dt = UtcDateTime::from_unix_seconds(secs);
        assert_eq!(dt.format_iso_utc(), iso, "{secs}s iso");
        assert_eq!(dt.to_string(), iso, "{secs}s display");
        assert_eq!(dt.format_compact_utc(), compact, "{secs}s compact");
        assert_eq!(dt.format_date(), date, "{secs}s date");
    }
}

#[test]
fn year_from_days_matches_epoch_leap_and_century_boundaries() {
    let cases = [
        (0_i64, 1_970_u16),
        (59, 1_970),      // 1 Mar 1970
        (10_957, 2_000),  // 1 Jan 2000 (10_957 days after 1970-01-01)
        (11_016, 2_000),  // 29 Feb 2000 (leap century)
        (19_723, 2_024),  // 1 Jan 2024
        (19_782, 2_024),  // 29 Feb 2024
        (19_783, 2_024),  // 1 Mar 2024
        (20_089, 2_025),  // 1 Jan 2025
        (-25_567, 1_900), // 1 Jan 1900 (non-leap century)
        (-2_556, 1_963),
        (45_999, 2_095),
        (-1_000_000, 0),
        (100_000_000, u16::MAX),
    ];
    for (days, expected) in cases {
        assert_eq!(year_from_days_since_epoch(days), expected, "{days} days");
    }
}

#[test]
fn parse_rfc3339_utc_handles_z_lowercase_fractions_and_offsets() {
    let plain = UtcDateTime::parse_rfc3339_utc("2025-01-15T12:30:05Z").expect("valid Z");
    assert_eq!(
        plain,
        UtcDateTime {
            year: 2025,
            month: 1,
            day: 15,
            hour: 12,
            minute: 30,
            second: 5,
        }
    );

    let lower_frac =
        UtcDateTime::parse_rfc3339_utc("2024-02-29t23:59:59.987654z").expect("leap + frac");
    assert_eq!(lower_frac.format_iso_utc(), "2024-02-29T23:59:59Z");

    let plus =
        UtcDateTime::parse_rfc3339_utc("2025-01-01T01:30:00+02:00").expect("positive offset");
    assert_eq!(plus.format_iso_utc(), "2024-12-31T23:30:00Z");

    let minus =
        UtcDateTime::parse_rfc3339_utc("2024-12-31T22:00:00-03:30").expect("negative offset");
    assert_eq!(minus.format_iso_utc(), "2025-01-01T01:30:00Z");

    let leap_century =
        UtcDateTime::parse_rfc3339_utc("2000-02-29T00:00:00Z").expect("2000 is leap");
    assert_eq!(leap_century.day, 29);

    let apr_30 = UtcDateTime::parse_rfc3339_utc("2025-04-30T23:59:59Z").expect("30-day month");
    assert_eq!(apr_30.day, 30);

    let max_plus =
        UtcDateTime::parse_rfc3339_utc("2025-01-15T00:00:00+23:59").expect("max positive offset");
    assert_eq!(max_plus.format_iso_utc(), "2025-01-14T00:01:00Z");

    let max_minus =
        UtcDateTime::parse_rfc3339_utc("2025-01-15T23:59:59-23:59").expect("max negative offset");
    assert_eq!(max_minus.format_iso_utc(), "2025-01-16T23:58:59Z");
}

#[test]
fn parse_rfc3339_utc_rejects_malformed_and_invalid_calendar_inputs() {
    let invalid = [
        "",
        "2025-01-15",
        "2025/01/15T12:00:00Z",
        "2025-01/15T12:00:00Z",
        "2025/01-15T12:00:00Z",
        "2025-01-15 12:00:00Z",
        "2025-01-15T12.00:00Z",
        "2025-00-15T12:00:00Z",
        "2025-13-15T12:00:00Z",
        "2025-01-00T12:00:00Z",
        "2025-01-32T12:00:00Z",
        "2025-04-31T12:00:00Z",
        "2025-02-29T12:00:00Z",
        "1900-02-29T12:00:00Z",
        "2025-01-15T24:00:00Z",
        "2025-01-15T12:60:00Z",
        "2025-01-15T12:00:60Z",
        "2025-01-15T12:00:00.Z",
        "2025-01-15T12:00:00.123",
        "2025-01-15T12:00:00+24:00",
        "2025-01-15T12:00:00+00:60",
        "2025-01-15T12:00:00+0000",
        "2025-01-15T12:00:00Zextra",
        "abcd-01-15T12:00:00Z",
        "2025-ab-15T12:00:00Z",
        "2025-0x-15T12:00:00Z",
        "2025-x1-15T12:00:00Z",
    ];
    for input in invalid {
        let err = UtcDateTime::parse_rfc3339_utc(input)
            .expect_err(&format!("expected failure for {input:?}"));
        assert!(
            err.to_string().contains("invalid RFC 3339 timestamp"),
            "unexpected error formatting for {input:?}: {err}"
        );
    }
}

#[test]
fn civil_round_trip_matches_every_leap_and_month_boundary() {
    let mut expected_days = days_from_civil(1899, 12, 31);
    for year in 1900..=2105_i32 {
        for month in 1..=12_u8 {
            for day in 1..=days_in_month(year, month) {
                expected_days += 1;
                let got_days = days_from_civil(i64::from(year), i64::from(month), i64::from(day));
                assert_eq!(got_days, expected_days, "{year}-{month}-{day}");
                let (ry, rm, rd) = civil_from_days(got_days);
                assert_eq!((ry, rm, rd), (year, month, day), "round-trip {got_days}");
            }
        }
    }
}
