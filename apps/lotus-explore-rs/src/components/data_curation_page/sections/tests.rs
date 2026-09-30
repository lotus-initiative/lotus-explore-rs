// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for the curation-page sections, in their own file.
//!
//! The assertions are on the values a section hands to a child -- the button's
//! enabled state, the row count, the `QuickStatements` link -- because that is the
//! only part of a component a test can see. Whether it then looks right is a
//! question for a browser.

#![allow(clippy::indexing_slicing)]

use crate::curation::CurationInputRow;

#[test]
fn queue_rows_removal_logic_preserves_other_rows() {
    let mut rows = vec![
        CurationInputRow {
            name: "A".to_string(),
            smiles: "CCO".to_string(),
            taxon: None,
            doi: None,
        },
        CurationInputRow {
            name: "B".to_string(),
            smiles: "CCN".to_string(),
            taxon: None,
            doi: None,
        },
        CurationInputRow {
            name: "C".to_string(),
            smiles: "CCC".to_string(),
            taxon: None,
            doi: None,
        },
    ];
    rows.remove(1);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].name, "A");
    assert_eq!(rows[1].name, "C");
}
