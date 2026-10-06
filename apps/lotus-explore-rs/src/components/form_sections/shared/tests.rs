// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `shared`, in their own file.

use super::{normalized_year_input_max, parse_f64_input, parse_u16_input};

#[test]
fn parse_f64_input_accepts_valid_numbers_and_rejects_invalid_text() {
    assert_eq!(parse_f64_input("42"), Some(42.0));
    assert_eq!(parse_f64_input("2.5"), Some(2.5));
    assert_eq!(parse_f64_input("abc"), None);
}

#[test]
fn parse_u16_input_accepts_positive_integers_only() {
    assert_eq!(parse_u16_input("007"), Some(7));
    assert_eq!(parse_u16_input("65535"), Some(u16::MAX));
    assert_eq!(parse_u16_input("-1"), None);
    assert_eq!(parse_u16_input("12.5"), None);
}

#[test]
fn normalized_year_input_max_never_drops_below_default_floor() {
    assert_eq!(normalized_year_input_max(2030), 2030);
    assert_eq!(
        normalized_year_input_max(lotus_model::YEAR_MIN),
        lotus_model::YEAR_MIN
    );
    assert_eq!(normalized_year_input_max(1700), lotus_model::YEAR_MIN);
}
