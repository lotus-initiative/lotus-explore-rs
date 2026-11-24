// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! RFC 4180 CSV formatting for [`RowExporter`](super::RowExporter).

use std::borrow::Cow;

pub(super) fn push_preamble(out: &mut String, columns: &[&str]) {
    out.push_str(&columns.join(","));
    out.push('\n');
}

pub(super) fn push_row<'a>(out: &mut String, cells: impl IntoIterator<Item = Cow<'a, str>>) {
    let mut first = true;
    for cell in cells {
        if !std::mem::replace(&mut first, false) {
            out.push(',');
        }
        out.push_str(&csv_field(cell.as_ref()));
    }
    out.push('\n');
}

/// A CSV field, quoted only when it has to be.
///
/// RFC 4180: a field containing a comma, a quote, CR or LF must be quoted, and an
/// embedded quote doubled. Quoting everything is also valid and simpler, but it makes
/// a spreadsheet import of a million numeric columns noticeably slower and the file
/// noticeably larger for no gain.
pub(super) fn csv_field(value: &str) -> Cow<'_, str> {
    let needs_quotes = value
        .chars()
        .any(|ch| matches!(ch, ',' | '"' | '\n' | '\r'));
    if !needs_quotes {
        return Cow::Borrowed(value);
    }
    let mut out = String::with_capacity(value.len() + 8);
    out.push('"');
    for ch in value.chars() {
        if ch == '"' {
            out.push('"');
        }
        out.push(ch);
    }
    out.push('"');
    Cow::Owned(out)
}
