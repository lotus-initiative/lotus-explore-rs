// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The emitters must stay inside the profiles they claim to conform to.
//!
//! Bioschemas validation does not fail on an extra property, so an invented one
//! is invisible to the only check the crate already had. Three have been emitted
//! and removed: `monoisotopicMolecularWeight` carrying the molecular mass, an
//! `inchikey` identifier beside the real `inChIKey` one, and a result hash
//! presented as an ARK.

#![allow(
    clippy::expect_used,
    reason = "a fixture that stopped being the expected shape is a broken test"
)]

use super::{CompoundEntry, Profile, compound_jsonld};
use std::sync::Arc;

fn a_compound_entry() -> CompoundEntry {
    CompoundEntry {
        compound_qid: Arc::from("Q1234"),
        name: Arc::from("Quercetin"),
        inchikey: Some(Arc::from("IIYFPWUAQGXNF-FGGY-VCPS-NA-NSA")),
        smiles: Some(Arc::from("c1ccccc1")),
        mass: Some(302.236),
        formula: Some(Arc::from("C15H10O7")),
        ref_doi: Some(Arc::from("10.1/A")),
        ..CompoundEntry::default()
    }
}

/// Properties the profile does not declare and the LOTUS projection has no
/// value for. Listed so adding one is a deliberate act.
const OUTSIDE_THE_PROFILE: &[&str] = &["monoisotopicMolecularWeight"];

#[test]
fn a_compound_emits_only_properties_the_profile_declares() {
    let node = compound_jsonld(&a_compound_entry());
    let declared = Profile::MolecularEntity.declared_properties();

    for key in node.as_object().expect("an object").keys() {
        // The stamp and the JSON-LD keys are structure, not assertions about
        // the entity: `stamp` adds `dct:conformsTo`, which is how the node
        // declares which profile it is written against.
        if key.starts_with('@') || key == "dct:conformsTo" {
            continue;
        }
        assert!(
            declared.contains(&key.as_str()) || matches!(key.as_str(), "sameAs" | "citation"),
            "{key} is not a property the MolecularEntity profile declares, and \
             nothing in the LOTUS projection backs it: {node}"
        );
        assert!(
            !OUTSIDE_THE_PROFILE.contains(&key.as_str()),
            "{key} was an invented addition and must stay out: {node}"
        );
    }
}

#[test]
fn an_identifier_is_emitted_once_per_property() {
    let node = compound_jsonld(&a_compound_entry());
    let ids = node
        .get("identifier")
        .and_then(serde_json::Value::as_array)
        .expect("identifier is an array");
    let keys: Vec<&str> = ids
        .iter()
        .filter_map(|v| v["propertyID"].as_str())
        .collect();
    let mut unique = keys.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        keys.len(),
        unique.len(),
        "duplicate identifier properties: {keys:?}"
    );
}
