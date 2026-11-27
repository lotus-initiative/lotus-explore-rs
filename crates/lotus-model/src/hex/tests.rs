// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Unit tests for `hex`.

use super::*;

#[test]
fn encodes_empty_and_known_vectors() {
    assert_eq!(encode_hex_lower(&[]), "");
    assert_eq!(
        encode_hex_lower(&[0x00, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xff]),
        "000123456789abcdefff"
    );
}

#[test]
fn push_hex_lower_appends_after_existing_prefix() {
    let mut buf = String::from("search:");
    push_hex_lower(&mut buf, &[0x0a, 0xf0]);
    assert_eq!(buf, "search:0af0");
}

#[test]
fn all_byte_values_match_std_lower_hex() {
    use std::fmt::Write as _;
    let mut expected = String::with_capacity(512);
    let bytes: Vec<u8> = (0..=u8::MAX).collect();
    for b in &bytes {
        let _ = write!(expected, "{b:02x}");
    }
    assert_eq!(encode_hex_lower(&bytes), expected);
}
