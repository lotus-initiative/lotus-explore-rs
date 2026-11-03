// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use std::borrow::Cow;
use std::fmt::Write as _;

const fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~')
}

/// Percent-encode `input` using the RFC 3986 unreserved character set
/// (`A-Z`, `a-z`, `0-9`, `-`, `_`, `.`, `~`).
///
/// Returns [`Cow::Borrowed`] when every byte of `input` is already unreserved.
#[must_use]
pub fn percent_encode(input: &str) -> Cow<'_, str> {
    if input.bytes().all(is_unreserved) {
        return Cow::Borrowed(input);
    }
    let mut out = String::new();
    for byte in input.bytes() {
        if is_unreserved(byte) {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    Cow::Owned(out)
}

/// Decode a percent-encoded UTF-8 string (`%XX` hex pairs).
///
/// Returns [`Cow::Borrowed`] when `input` contains no `%` byte. Any `%` not
/// followed by two ASCII hex digits is preserved verbatim.
///
/// # Errors
///
/// Returns [`std::string::FromUtf8Error`] when the decoded bytes are not valid
/// UTF-8.
pub fn percent_decode(input: &str) -> Result<Cow<'_, str>, std::string::FromUtf8Error> {
    if !input.as_bytes().contains(&b'%') {
        return Ok(Cow::Borrowed(input));
    }
    let mut out = Vec::new();
    let mut bytes = input.as_bytes();
    while let Some((&first, rest)) = bytes.split_first() {
        if first == b'%'
            && let [hi, lo, tail @ ..] = rest
            && hi.is_ascii_hexdigit()
            && let Ok(hex) = std::str::from_utf8(&[*hi, *lo])
            && let Ok(decoded) = u8::from_str_radix(hex, 16)
        {
            out.push(decoded);
            bytes = tail;
        } else {
            out.push(first);
            bytes = rest;
        }
    }
    String::from_utf8(out).map(Cow::Owned)
}

#[cfg(test)]
#[path = "percent/tests.rs"]
mod tests;
