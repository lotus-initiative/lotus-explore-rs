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
        // `inChIKey` is the property the profile names. A second identifier under
        // a made-up `inchikey` propertyID, typed with an IUPAC vocabulary IRI as a
        // valueType, was also emitted: two identifiers for one value, neither of
        // them named by the profile. It is gone.
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
        // Only `molecularWeight`. The row's mass is Wikidata `P2067`, which is the
        // molecular mass and *not* the monoisotopic mass, and it was also being
        // emitted under `monoisotopicMolecularWeight` with the same number. A second
        // property asserting a quantity the data does not carry, carrying it wrong,
        // is worse than the property being absent.
        object.insert("molecularWeight".into(), json!(mass));
    }
    if let Some(doi) = entry.ref_doi.as_deref() {
        object.insert("citation".into(), json!(doi_uri(doi)));
    }

    crate::stamp(Value::Object(object), Profile::MolecularEntity)
}

#[cfg(test)]
#[path = "compound/tests.rs"]
mod tests;
