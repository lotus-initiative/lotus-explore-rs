// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `lib`, in their own file.

#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing
)]

use super::*;
#[test]
fn each_uri_carries_its_own_identifier() {
    assert_eq!(wikidata_uri("Q1"), "http://www.wikidata.org/entity/Q1");
    assert_eq!(
        pubchem_uri("LFQSCWFLJHTTHZ-UHFFFAOYSA-N"),
        "https://pubchem.ncbi.nlm.nih.gov/compound/LFQSCWFLJHTTHZ-UHFFFAOYSA-N"
    );
    assert_eq!(doi_uri("10.1000/xyz"), "https://doi.org/10.1000/xyz");
}

#[test]
fn a_property_value_is_shaped_the_way_a_consumer_expects() {
    let value = property_value(
        "wikidata",
        "http://www.wikidata.org/entity/Q1",
        "uri",
        "http://wikiba.se/ontology#WikidataItem",
    );
    assert_eq!(value["@type"], "PropertyValue");
    assert_eq!(value["propertyID"], "wikidata");
    assert_eq!(value["valueType"], "uri");
    assert_eq!(value["value"], "http://www.wikidata.org/entity/Q1");
    assert_eq!(
        value["identifier"], "http://wikiba.se/ontology#WikidataItem",
        "the scheme travels as the identifier, which is where a consumer reads it"
    );
}

#[test]
fn an_entity_is_returned_for_a_row() {
    let entry = lotus_model::CompoundEntry {
        compound_qid: std::sync::Arc::from("Q1"),
        ..lotus_model::CompoundEntry::default()
    };
    let entity = entity_for(&entry).expect("infallible today");
    assert_eq!(entity["@id"], "http://www.wikidata.org/entity/Q1");
}
