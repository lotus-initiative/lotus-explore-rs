// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! A compound as a Bioschemas `MolecularEntity`.
//!
//! The `LOTUS` projection knows a compound's `InChIKey`, `SMILES`, mass, formula
//! and label, and Wikidata knows its QID. That is enough for the profile's four
//! required properties, which is why this builds from a result row directly.

use crate::{Profile, doi_uri, property_value, pubchem_uri, wikidata_uri};
use lotus_model::CompoundEntry;
use serde_json::{Value, json};

/// The identifiers a row contributes, as `identifier` and `sameAs` values.
fn identifiers(entry: &CompoundEntry, entity: &str) -> Vec<Value> {
    let mut out = vec![property_value(
        "wikidata",
        entity,
        "uri",
        "http://wikiba.se/ontology#WikidataItem",
    )];
    if let Some(key) = entry.inchikey.as_deref() {
        out.push(property_value(
            "inchikey",
            key,
            "string",
            "https://www.iupac.org/inchi",
        ));
        // The profile names the property `inChIKey`, so the identifier uses that
        // spelling rather than Wikidata's P235.
        out.push(json!({"@type": "PropertyValue", "propertyID": "inChIKey", "value": key}));
    }
    out
}

/// The `MolecularEntity` for a result row.
///
/// A row with no QID yields `null`: a `MolecularEntity` without an identifier
/// is not a conforming one, and the caller needs to know to skip it rather than
/// publish a node that fails validation.
#[must_use]
pub fn compound_jsonld(entry: &CompoundEntry) -> Value {
    let qid = entry.compound_qid.trim();
    if qid.is_empty() {
        return Value::Null;
    }
    let entity = wikidata_uri(qid);

    let mut object = serde_json::Map::new();
    object.insert("@type".into(), json!("MolecularEntity"));
    object.insert("@id".into(), json!(entity));
    object.insert(
        "name".into(),
        json!(if entry.name.is_empty() {
            qid
        } else {
            &*entry.name
        }),
    );
    object.insert("url".into(), json!(entity));
    object.insert("identifier".into(), json!(identifiers(entry, &entity)));

    let mut same_as = vec![json!(entity)];
    if let Some(key) = entry.inchikey.as_deref() {
        // PubChem is keyed on the InChIKey, so it is a resolvable `sameAs`.
        same_as.push(json!(pubchem_uri(key)));
        object.insert("inChIKey".into(), json!(key));
    }
    object.insert("sameAs".into(), json!(same_as));

    if let Some(smiles) = entry.smiles.as_deref() {
        object.insert("smiles".into(), json!(smiles));
    }
    if let Some(formula) = entry.formula.as_deref() {
        object.insert("molecularFormula".into(), json!(formula));
    }
    if let Some(mass) = entry.mass {
        object.insert("molecularWeight".into(), json!(mass));
        object.insert("monoisotopicMolecularWeight".into(), json!(mass));
    }
    if let Some(doi) = entry.ref_doi.as_deref() {
        object.insert("citation".into(), json!(doi_uri(doi)));
    }

    crate::stamp(Value::Object(object), Profile::MolecularEntity)
}

#[cfg(test)]
mod tests {
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
}
