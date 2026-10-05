// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

mod compound;
mod dataset;
mod profile;
mod software;
mod taxon;

pub use compound::compound_jsonld;
pub use dataset::{ResultSet, dataset_jsonld, result_set_jsonld};
pub use profile::{Profile, ValidationIssue, check};
pub use software::{Software, citation_cff, codemeta, software_jsonld};
pub use taxon::taxon_jsonld;

use lotus_model::CompoundEntry;

/// The `@context` every document here shares.
pub const CONTEXT: &str = "https://schema.org/";

/// Stamps `@context` and `dct:conformsTo` onto a node.
///
/// Both together, because a JSON-LD document that carries a context but does not
/// say which profile it claims is not checkable: a validator has no way to know
/// which set of required properties applies. Bioschemas lists `dct:conformsTo`
/// as mandatory for this reason, and the previous version of this crate emitted
/// the context without it.
///
/// The value is the profile's own `@id`, so a consumer can resolve the exact
/// profile version rather than guessing from the shape of the document.
pub(crate) fn stamp(mut node: serde_json::Value, profile: Profile) -> serde_json::Value {
    // A no-op for a document that is not an object, rather than a panic:
    // indexing a `Value` panics on a non-object, and this is called on
    // whatever a caller built.
    if let Some(object) = node.as_object_mut() {
        object.insert("@context".into(), CONTEXT.into());
        object.insert("dct:conformsTo".into(), profile.id().into());
    }
    node
}

/// The Wikidata entity URI for a QID.
#[must_use]
pub fn wikidata_uri(qid: &str) -> String {
    format!("http://www.wikidata.org/entity/{qid}")
}

/// The canonical URL for a LOTUS concept, as a `sameAs` target.
#[must_use]
pub fn pubchem_uri(inchikey: &str) -> String {
    format!("https://pubchem.ncbi.nlm.nih.gov/compound/{inchikey}")
}

/// The `doi.org` URL for a bare DOI.
#[must_use]
pub fn doi_uri(doi: &str) -> String {
    format!("https://doi.org/{doi}")
}

/// A `PropertyValue` pairing a value with the scheme that identifies it.
///
/// Wikidata's `P235` and friends have no schema.org equivalent, so the
/// identifier goes in `identifier` with its own vocabulary, which is what the
/// Bioschemas `MolecularEntity` profile asks for.
#[must_use]
pub fn property_value(
    property: &str,
    value: &str,
    value_type: &str,
    scheme: &str,
) -> serde_json::Value {
    serde_json::json!({
        "@type": "PropertyValue",
        "propertyID": property,
        "value": value,
        "valueType": value_type,
        "identifier": scheme,
    })
}

/// The `Dataset` every result belongs to, given a result set's identity.
#[must_use]
pub fn result_dataset_url(name_slug: &str, query_hash: &str) -> String {
    format!("https://lotus.nprod.net/lotus-explore-rs/dataset/{name_slug}/{query_hash}")
}

/// The project itself, for a page that describes the software.
pub const SOFTWARE: crate::software::Software = crate::software::Software {
    name: "LOTUS Explorer",
    description: "A linked open data explorer for the LOTUS compound-taxon-reference \
                  knowledge graph in Wikidata, queried over SPARQL.",
    url: "https://lotus.nprod.net/lotus-explore-rs",
    repository: env!("CARGO_PKG_REPOSITORY"),
    issue_tracker: concat!(env!("CARGO_PKG_REPOSITORY"), "/issues"),
    doi: Some("10.7554/eLife.70780"),
    version: env!("CARGO_PKG_VERSION"),
    license: "AGPL-3.0-only",
    license_url: "https://www.gnu.org/licenses/agpl-3.0.html",
    language: "Rust",
    language_url: "https://www.rust-lang.org",
    requires_rust: ">= 1.97",
    // The first commit in this repository; there is no release tag to date from yet.
    date_published: "2026-09-22",
    application_category: "scientific data analysis",
    application_subcategory: "SPARQL client",
    features: &[
        "SPARQL search over the LOTUS projection in Wikidata",
        "substructure and similarity structure search",
        "taxon, mass, year and molecular formula filters",
        "Bioschemas-compliant JSON-LD export",
        "QuickStatements curation for compounds Wikidata is missing",
    ],
    keywords: &[
        "natural products",
        "linked open data",
        "SPARQL",
        "bioinformatics",
        "chemical compound",
        "natural product",
        "taxonomy",
        "chemotaxonomy",
        "curation",
        "rust",
    ],
    paper: Some(crate::software::Paper {
        title: "The LOTUS initiative for open knowledge management in natural products research",
        doi: "10.7554/eLife.70780",
        year: 2022,
    }),
};

/// The `MolecularEntity` for a result row, or `None` if the row has no QID.
///
/// # Errors
/// Never, today. It returns `Result` so that a future profile check can fail
/// without changing every call site's type.
pub fn entity_for(entry: &CompoundEntry) -> Result<serde_json::Value, serde_json::Error> {
    Ok(compound_jsonld(entry))
}

#[cfg(test)]
mod uri_tests {
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
}

#[cfg(test)]
mod profile_bounds_tests {
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
}
