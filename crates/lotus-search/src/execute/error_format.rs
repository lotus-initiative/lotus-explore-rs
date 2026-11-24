// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Error body compaction, JSON/HTML title extraction, and form-URL encoding.

use std::fmt::Write as _;

/// Reduce an error body to one line, preferring the endpoint's own `exception`
/// or `wdqs-proxy` `message` field over an HTML gateway page.
pub(super) fn compact(body: &[u8]) -> String {
    const MAX: usize = 240;
    let text = String::from_utf8_lossy(body);
    let line = json_exception(&text)
        .or_else(|| json_string_field(&text, "message"))
        .or_else(|| html_title(&text))
        .or_else(|| {
            text.lines()
                .map(str::trim)
                .find(|l| !l.is_empty() && !matches!(*l, "{" | "}"))
        })
        .unwrap_or(&text)
        .trim_matches(',');

    if line.chars().count() <= MAX {
        return line.to_string();
    }
    let truncated: String = line.chars().take(MAX).collect();
    format!("{truncated}…")
}

/// The `<title>` of an error page, which is where a gateway puts the reason
/// (`502 Bad Gateway`). The markup around it says nothing.
#[must_use]
pub(super) fn html_title(text: &str) -> Option<&str> {
    if !text.contains('<') {
        return None;
    }
    let rest = text.split_once("<title>")?.1;
    let end = rest.find("</title>")?;
    let title = rest[..end].trim();
    (!title.is_empty()).then_some(title)
}

/// Pull `"exception": "…"` out of `QLever`'s error format.
pub(super) fn json_exception(text: &str) -> Option<&str> {
    json_string_field(text, "exception")
}

/// Pull `"<key>": "…"` out of a JSON error body. Hand-rolled, because the
/// alternative is a JSON dependency for one field of one error path.
pub(super) fn json_string_field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{key}\"");
    let rest = text.split_once(&needle)?.1.trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;

    let mut end = 0;
    let mut chars = rest.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => {
                chars.next();
            }
            '"' => return Some(&rest[..end.max(i)]),
            _ => end = i + c.len_utf8(),
        }
    }
    None
}

/// Percent-encode for `application/x-www-form-urlencoded`.
pub(super) fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(*byte));
            }
            b' ' => out.push('+'),
            other => {
                out.push('%');
                let _ = write!(out, "{other:02X}");
            }
        }
    }
    out
}
