// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `compound`, in their own file.

#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use super::*;
use crate::{Profile, check};
use std::sync::Arc;

fn entry() -> CompoundEntry {
    CompoundEntry {
        compound_qid: Arc::from("Q1234"),
        name: Arc::from("Quercetin"),
        inchikey: Some(Arc::from("IIYFPWUAQGXNF-FGGY-VCPS-NA-NSA")),
        smiles: Some(Arc::from("O=c1c(O)c(-c2ccc(O)c(O)c2)oc2cc(O)cc(O)c12")),
        mass: Some(302.236),
        formula: Some(Arc::from("C15H10O7")),
        ref_doi: Some(Arc::from("10.1/A")),
        ..CompoundEntry::default()
    }
}

#[test]
fn a_row_becomes_a_conforming_molecular_entity() {
    let node = compound_jsonld(&entry());
    let issues = check(&node, Profile::MolecularEntity);
    assert!(
        !issues.iter().any(|i| i.severity == "Violation"),
        "violations: {issues:?}"
    );
}

#[test]
fn the_wikidata_qid_is_both_the_id_and_a_same_as() {
    let node = compound_jsonld(&entry());
    assert_eq!(node["@id"], json!("http://www.wikidata.org/entity/Q1234"));
    assert!(
        node["sameAs"]
            .as_array()
            .expect("an array")
            .contains(&json!("http://www.wikidata.org/entity/Q1234"))
    );
}

#[test]
fn the_inchikey_resolves_to_pubchem() {
    let node = compound_jsonld(&entry());
    assert!(
        node["sameAs"]
            .as_array()
            .expect("an array")
            .contains(&json!(pubchem_uri("IIYFPWUAQGXNF-FGGY-VCPS-NA-NSA")))
    );
}

#[test]
fn every_optional_property_the_row_knows_is_carried() {
    let node = compound_jsonld(&entry());
    assert_eq!(
        node["smiles"],
        entry().smiles.as_deref().map_or(Value::Null, |s| json!(s))
    );
    assert_eq!(node["molecularFormula"], json!("C15H10O7"));
    assert_eq!(node["molecularWeight"], json!(302.236));
    assert_eq!(node["citation"], json!("https://doi.org/10.1/A"));
}

#[test]
fn a_sparse_row_still_conforms_on_the_required_properties() {
    let sparse = CompoundEntry {
        compound_qid: Arc::from("Q9"),
        ..CompoundEntry::default()
    };
    let node = compound_jsonld(&sparse);
    let issues = check(&node, Profile::MolecularEntity);
    assert!(
        !issues.iter().any(|i| i.severity == "Violation"),
        "a compound with only a QID still conforms: {issues:?}"
    );
    // The profile's recommendations it cannot meet are warnings.
    assert!(issues.iter().all(|i| i.severity == "Warning"));
}

#[test]
fn a_row_with_no_qid_yields_nothing_rather_than_an_invalid_node() {
    let row = CompoundEntry::default();
    assert!(compound_jsonld(&row).is_null());
}

#[test]
fn the_type_is_the_one_the_profile_constrains() {
    assert_eq!(compound_jsonld(&entry())["@type"], json!("MolecularEntity"));
}

#[test]
fn a_row_always_contributes_at_least_its_wikidata_identifier() {
    // The vector is seeded with the Wikidata property rather than built by
    // pushing, so a row with no inchikey, no smiles and no DOI still carries
    // the one identifier that identifies it. An empty list here would emit a
    // compound with no identity at all.
    let entry = CompoundEntry {
        compound_qid: Arc::from("Q1"),
        ..CompoundEntry::default()
    };
    let out = identifiers(&entry, "http://www.wikidata.org/entity/Q1");
    assert_eq!(out.len(), 1, "the Wikidata identifier is always there");
    assert_eq!(out[0]["propertyID"], "wikidata");
}
