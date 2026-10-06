// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `pipeline`, in their own file.

#![allow(clippy::expect_used)]

use super::*;
use futures::executor::block_on;
use lotus_curation::{CurationInputRow, CurationStatus};

fn key(row: &CurationInputRow) -> String {
    row.smiles.to_ascii_lowercase()
}

async fn fake_curate(_locale: Locale, input: CurationInputRow) -> CurationResultRow {
    CurationResultRow {
        input,
        canonical_smiles: None,
        inchikey: None,
        inchi: None,
        formula: None,
        exact_mass: None,
        mass_warning: None,
        wikidata_qid: None,
        status: CurationStatus::NewCompound,
        note: String::new(),
        dependency_blocks: Vec::new(),
        quickstatements: vec!["CREATE".to_string()],
    }
}

#[test]
fn curate_rows_deduplicates_and_preserves_input_order() {
    let rows = vec![
        CurationInputRow {
            name: "a".into(),
            smiles: "CCO".into(),
            taxon: None,
            doi: None,
        },
        CurationInputRow {
            name: "dup".into(),
            smiles: "cco".into(),
            taxon: None,
            doi: None,
        },
        CurationInputRow {
            name: "b".into(),
            smiles: "N".into(),
            taxon: None,
            doi: None,
        },
    ];

    let (result_rows, _bundle) =
        block_on(curate_rows(Locale::En, rows, fake_curate, key)).expect("pipeline result");
    let names = result_rows
        .into_iter()
        .map(|r| r.input.name)
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["a", "b"]);
}
