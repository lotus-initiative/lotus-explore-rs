// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Unit tests for the CSV delimiter lookup table.

use super::is_delimiter;

#[test]
fn csv_splitter_delimiter_table_only_marks_rfc4180_bytes() {
    for &delim in b"\",\n\r" {
        assert!(is_delimiter(delim), "expected delimiter {delim}");
    }
    for &ordinary in b"aZ0 \t;" {
        assert!(!is_delimiter(ordinary), "unexpected delimiter {ordinary}");
    }
}
