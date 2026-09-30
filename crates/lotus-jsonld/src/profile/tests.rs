// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for the Bioschemas profile checks, in their own file.
//!
//! Both directions matter: a conforming document must produce no issues, and a
//! document missing a required property must produce the one that is missing.

#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use super::*;
use serde_json::json;

fn conforming(profile: Profile) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    object.insert("@type".into(), json!(profile.rdf_type()));
    for property in profile.required().iter().chain(profile.recommended()) {
        object.insert((*property).into(), json!("x"));
    }
    crate::stamp(serde_json::Value::Object(object), profile)
}

#[test]
fn a_conforming_document_has_no_issues() {
    for profile in Profile::all() {
        let issues = check(&conforming(*profile), *profile);
        assert!(issues.is_empty(), "{}: {issues:?}", profile.id());
    }
}

#[test]
fn a_missing_required_property_is_a_violation() {
    let mut document = conforming(Profile::Dataset);
    document
        .as_object_mut()
        .expect("an object")
        .remove("license");

    let issues = check(&document, Profile::Dataset);
    let licence = issues
        .iter()
        .find(|i| i.property == "license")
        .expect("reported");
    assert_eq!(licence.severity, "Violation");
}

#[test]
fn a_document_must_declare_the_profile_it_claims() {
    // This is the check that was missing, and it is the one that matters
    // for a document from outside this crate: without `dct:conformsTo` a
    // consumer cannot tell which profile's required properties apply, so it
    // cannot check anything.
    let mut document = conforming(Profile::MolecularEntity);
    document
        .as_object_mut()
        .expect("an object")
        .remove("dct:conformsTo");

    let issues = check(&document, Profile::MolecularEntity);
    let declared = issues
        .iter()
        .find(|i| i.property == "dct:conformsTo")
        .expect("reported");
    assert_eq!(declared.severity, "Violation");
}

#[test]
fn a_document_claiming_a_different_profile_is_reported() {
    let mut document = conforming(Profile::MolecularEntity);
    document["dct:conformsTo"] = json!(Profile::Dataset.id());

    let issues = check(&document, Profile::MolecularEntity);
    let declared = issues
        .iter()
        .find(|i| i.property == "dct:conformsTo")
        .expect("reported");
    assert_eq!(declared.severity, "Violation");
    assert!(
        declared.message.contains(Profile::Dataset.id()),
        "the message should name the profile claimed: {}",
        declared.message
    );
}

#[test]
fn a_document_without_a_context_is_reported() {
    let mut document = conforming(Profile::MolecularEntity);
    document
        .as_object_mut()
        .expect("an object")
        .remove("@context");

    let issues = check(&document, Profile::MolecularEntity);
    assert!(
        issues.iter().any(|i| i.property == "@context"),
        "a JSON-LD document with no context means nothing: {issues:?}"
    );
}

#[test]
fn a_missing_recommended_property_is_only_a_warning() {
    let mut document = conforming(Profile::MolecularEntity);
    document
        .as_object_mut()
        .expect("an object")
        .remove("smiles");

    let issues = check(&document, Profile::MolecularEntity);
    let smiles = issues
        .iter()
        .find(|i| i.property == "smiles")
        .expect("reported");
    assert_eq!(smiles.severity, "Warning");
    assert!(
        !issues.iter().any(|i| i.severity == "Violation"),
        "and nothing else is broken"
    );
}

#[test]
fn an_empty_value_counts_as_missing() {
    // A `keywords: []` or an empty `url` satisfies nothing, and a document
    // that carries them is worse than one that admits the gap.
    let document = json!({
        "@type": "Dataset",
        "name": "x",
        "url": "",
        "keywords": [],
        "description": "d",
        "identifier": "i",
        "license": "l",
    });

    let issues = check(&document, Profile::Dataset);
    assert!(issues.iter().any(|i| i.property == "url"), "url is empty");
    assert!(
        issues.iter().any(|i| i.property == "keywords"),
        "keywords is empty"
    );
}

#[test]
fn the_wrong_type_is_reported_against_the_profile() {
    let document = json!({ "@type": "Thing", "name": "x" });
    let issues = check(&document, Profile::Dataset);
    assert!(issues.iter().any(|i| i.property == "@type"));
}

#[test]
fn a_non_object_document_is_reported_rather_than_panicking() {
    for bad in [json!(null), json!("x"), json!([]), json!(3)] {
        let issues = check(&bad, Profile::Dataset);
        assert!(!issues.is_empty(), "{bad}");
    }
}

#[test]
fn each_json_kind_is_named_in_its_own_words() {
    use serde_json::{Value, json};

    // These strings go into a validation message. Naming two kinds the same
    // way turns "expected a string, got a number" into a message that cannot
    // be acted on.
    assert_eq!(kind_of(&Value::Null), "null");
    assert_eq!(kind_of(&Value::Bool(true)), "a boolean");
    assert_eq!(kind_of(&Value::from(1)), "a number");
    assert_eq!(kind_of(&Value::from("s")), "a string");
    assert_eq!(kind_of(&Value::from(vec![1])), "an array");
    assert_eq!(kind_of(&json!({"a": 1})), "an object");
}
