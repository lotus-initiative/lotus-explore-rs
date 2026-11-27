// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! SPARQL Results JSON formatting for [`RowExporter`](super::RowExporter).

use std::borrow::Cow;
use std::fmt::Write as _;

pub(super) fn push_preamble(out: &mut String, columns: &[&str]) {
    // SPARQL Results JSON, so the same reader handles this as an
    // endpoint's answer. `head` before `results` is required by the format.
    out.push_str("{\"head\":{\"vars\":[");
    for (index, name) in columns.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        let _ = write!(out, "\"{name}\"");
    }
    out.push_str("]},\"results\":{\"bindings\":[");
}

pub(super) fn push_epilogue(out: &mut String) {
    out.push_str("]}}");
}

pub(super) fn push_row<'a>(
    out: &mut String,
    row: usize,
    columns: &[&str],
    cells: impl IntoIterator<Item = Cow<'a, str>>,
) {
    if row > 0 {
        out.push(',');
    }
    out.push('{');
    let mut first = true;
    for (name, cell) in columns.iter().zip(cells) {
        let value = cell.as_ref();
        if value.is_empty() {
            continue;
        }
        if !std::mem::replace(&mut first, false) {
            out.push(',');
        }
        let _ = write!(out, "\"{name}\":{{\"type\":\"literal\",\"value\":");
        json_string(out, value);
        out.push('}');
    }
    out.push('}');
}

/// Append `value` as a JSON string, quotes included.
pub(super) fn json_string(out: &mut String, value: &str) {
    super::push_escaped_quoted(out, value);
}
