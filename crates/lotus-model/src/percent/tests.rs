// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

#![allow(clippy::expect_used)]

use std::borrow::Cow;

use super::{percent_decode, percent_encode};

#[test]
fn unreserved_input_borrows_without_allocating() {
    let sample = "AZaz09-_.~";
    let encoded = percent_encode(sample);
    assert!(
        matches!(encoded, Cow::Borrowed(s) if s == sample),
        "RFC 3986 unreserved characters must borrow without allocating"
    );
    let decoded = percent_decode(sample).expect("ASCII is valid UTF-8");
    assert!(
        matches!(decoded, Cow::Borrowed(s) if s == sample),
        "input without '%' must borrow without allocating"
    );
}

#[test]
fn reserved_and_utf8_bytes_encode_to_uppercase_hex() {
    let encoded = percent_encode("10.1002/andp.18280880206?q=a b&c= café");
    assert!(
        matches!(encoded, Cow::Owned(_)),
        "reserved bytes must allocate an owned string"
    );
    assert_eq!(
        encoded, "10.1002%2Fandp.18280880206%3Fq%3Da%20b%26c%3D%20caf%C3%A9",
        "slashes, delimiters, spaces, and UTF-8 bytes must percent-encode as uppercase hex"
    );
}

#[test]
fn percent_decode_round_trips_mixed_case_hex_and_preserves_malformed_percent() {
    let decoded = percent_decode("10.1002%2fandp%2F1828%20caf%c3%A9%25")
        .expect("valid UTF-8 percent-encoded string");
    assert!(
        matches!(decoded, Cow::Owned(_)),
        "input containing '%' must return Cow::Owned"
    );
    assert_eq!(
        decoded, "10.1002/andp/1828 café%",
        "both lowercase and uppercase hex pairs must decode"
    );

    let malformed = percent_decode("100% raw %GG %+A %1 %").expect("fallback bytes are ASCII");
    assert_eq!(
        malformed, "100% raw %GG %+A %1 %",
        "incomplete or non-hex percent sequences including leading '+' must stay literal"
    );
}

#[test]
fn percent_decode_rejects_invalid_utf8_sequences() {
    let err = percent_decode("%FF%FE").expect_err("0xFF 0xFE is not valid UTF-8");
    assert_eq!(
        err.as_bytes(),
        &[0xFF, 0xFE],
        "error must preserve the offending decoded byte sequence"
    );
}
