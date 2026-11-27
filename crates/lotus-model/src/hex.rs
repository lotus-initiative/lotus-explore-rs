// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Lowercase hexadecimal formatting without slice indexing.

const fn nibble_to_hex(nibble: u8) -> char {
    let low = nibble & 0x0f;
    if low < 10 {
        (b'0' + low) as char
    } else {
        (b'a' + (low - 10)) as char
    }
}

/// Append lowercase hexadecimal digits for every byte in `bytes` onto `out`.
pub fn push_hex_lower(out: &mut String, bytes: &[u8]) {
    out.reserve(bytes.len() * 2);
    for &byte in bytes {
        out.push(nibble_to_hex(byte >> 4));
        out.push(nibble_to_hex(byte));
    }
}

/// Format `bytes` as a lowercase hexadecimal string.
#[must_use]
pub fn encode_hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    push_hex_lower(&mut out, bytes);
    out
}

#[cfg(test)]
#[path = "hex/tests.rs"]
mod tests;
