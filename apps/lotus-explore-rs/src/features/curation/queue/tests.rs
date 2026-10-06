// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `queue`, in their own file.

#![allow(clippy::indexing_slicing)]

use super::*;

#[test]
fn non_empty_trimmed_returns_none_for_blank_values() {
    assert_eq!(non_empty_trimmed("   \n\t"), None);
}

#[test]
fn non_empty_trimmed_keeps_meaningful_text() {
    assert_eq!(
        non_empty_trimmed("  Gentiana lutea  "),
        Some("Gentiana lutea".into())
    );
}

#[test]
fn append_unique_rows_skips_existing_and_duplicate_candidates() {
    let mut queue = vec![CurationInputRow {
        name: "A".into(),
        smiles: "CCO".into(),
        taxon: Some("Taxon".into()),
        doi: Some("10.1/ABC".into()),
    }];

    let outcome = append_unique_rows(
        &mut queue,
        vec![
            CurationInputRow {
                name: "A-duplicate-1".into(),
                smiles: "CCO".into(),
                taxon: Some("taxon".into()),
                doi: Some("https://doi.org/10.1/abc".into()),
            },
            CurationInputRow {
                name: "B".into(),
                smiles: "CCN".into(),
                taxon: None,
                doi: None,
            },
            CurationInputRow {
                name: "B-duplicate-2".into(),
                smiles: "CCN".into(),
                taxon: None,
                doi: None,
            },
        ],
    );

    assert_eq!(
        outcome,
        AppendOutcome {
            added: 1,
            skipped: 2
        }
    );
    assert_eq!(queue.len(), 2);
    assert_eq!(queue[1].name, "B");
}
