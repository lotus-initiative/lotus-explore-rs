// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `workflow`, in their own file.

#![allow(clippy::indexing_slicing)]

use super::*;

fn row(name: &str, smiles: &str, deps: Vec<&str>, qs: Vec<&str>) -> CurationResultRow {
    CurationResultRow {
        input: CurationInputRow {
            name: name.to_string(),
            smiles: smiles.to_string(),
            taxon: None,
            doi: None,
        },
        canonical_smiles: None,
        inchikey: None,
        inchi: None,
        formula: None,
        exact_mass: None,
        mass_warning: None,
        wikidata_qid: None,
        status: if deps.is_empty() {
            CurationStatus::NewCompound
        } else {
            CurationStatus::PendingDependencies
        },
        note: String::new(),
        dependency_blocks: deps.into_iter().map(str::to_string).collect(),
        quickstatements: qs.into_iter().map(str::to_string).collect(),
    }
}

#[test]
fn format_curation_error_maps_rate_limit_messages() {
    let msg = format_curation_error(Locale::En, "HTTP 429 from service");
    assert!(msg.to_ascii_lowercase().contains("rate"));
}

#[test]
fn second_pass_inputs_only_keeps_rows_with_dependencies() {
    let rows = vec![
        CurationResultRow {
            input: CurationInputRow {
                name: "A".into(),
                smiles: "CCO".into(),
                taxon: None,
                doi: None,
            },
            canonical_smiles: None,
            inchikey: None,
            inchi: None,
            formula: None,
            exact_mass: None,
            mass_warning: None,
            wikidata_qid: None,
            status: CurationStatus::NewCompound,
            note: String::new(),
            dependency_blocks: vec![],
            quickstatements: vec![],
        },
        CurationResultRow {
            input: CurationInputRow {
                name: "B".into(),
                smiles: "CCN".into(),
                taxon: None,
                doi: None,
            },
            canonical_smiles: None,
            inchikey: None,
            inchi: None,
            formula: None,
            exact_mass: None,
            mass_warning: None,
            wikidata_qid: None,
            status: CurationStatus::PendingDependencies,
            note: String::new(),
            dependency_blocks: vec!["DEP".into()],
            quickstatements: vec![],
        },
    ];

    let inputs = second_pass_inputs(&rows);
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].name, "B");
}

#[test]
fn apply_second_pass_updates_matching_rows_and_keeps_order() {
    let previous = vec![
        row("A", "CCO", vec!["DEP-A"], vec![]),
        row("B", "CCN", vec![], vec!["MAIN-B"]),
    ];
    let updated = vec![
        row("A", "CCO", vec![], vec!["MAIN-A"]),
        row("C", "CCC", vec![], vec!["MAIN-C"]),
    ];

    let outcome = apply_second_pass(Locale::En, &previous, updated);

    assert_eq!(outcome.result_rows.len(), 2);
    assert_eq!(outcome.result_rows[0].input.name, "A");
    assert_eq!(
        outcome.result_rows[0].dependency_blocks.len(),
        0,
        "expected no entries"
    );
    assert_eq!(outcome.result_rows[0].quickstatements, vec!["MAIN-A"]);
    assert_eq!(outcome.result_rows[1].input.name, "B");
}

#[test]
fn apply_second_pass_marks_done_when_dependencies_cleared() {
    let previous = vec![row("A", "CCO", vec!["DEP-A"], vec![])];
    let updated = vec![row("A", "CCO", vec![], vec!["MAIN-A"])];

    let outcome = apply_second_pass(Locale::En, &previous, updated);

    assert!(!outcome.awaiting_second_pass);
    assert_eq!(outcome.status_message, msg_second_pass_done(Locale::En));
    assert_eq!(
        outcome.quickstatements.dependencies.len(),
        0,
        "expected no entries"
    );
    assert_eq!(outcome.quickstatements.main.as_ref(), "MAIN-A");
}
