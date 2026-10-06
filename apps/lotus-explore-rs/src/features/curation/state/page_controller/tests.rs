// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `page_controller`, in their own file.

use super::*;

fn clean_tsv_cell(value: &str) -> String {
    let mut buf = String::with_capacity(value.len());
    push_tsv_cell(&mut buf, value);
    buf
}

#[test]
fn clean_tsv_cell_normalizes_multiline_and_tabs() {
    assert_eq!(clean_tsv_cell("  A\tB\nC\r "), "A B C");
}

#[test]
fn rows_to_tsv_writes_expected_header_and_cells() {
    let rows = vec![CurationInputRow {
        name: "A\nname".into(),
        smiles: "CCO".into(),
        taxon: Some("Tax\ton".into()),
        doi: Some("10.1/ABC".into()),
    }];

    let tsv = rows_to_tsv(&rows);
    assert!(tsv.starts_with("name\tsmiles\ttaxon\tdoi\n"));
    assert!(tsv.contains("A name\tCCO\tTax on\t10.1/ABC\n"));
}

#[test]
fn should_autorun_only_when_pending_with_rows_and_no_results() {
    assert!(should_autorun(true, 2, false, 0));
    assert!(!should_autorun(false, 2, false, 0));
    assert!(!should_autorun(true, 0, false, 0));
    assert!(!should_autorun(true, 2, true, 0));
    assert!(!should_autorun(true, 2, false, 1));
}
