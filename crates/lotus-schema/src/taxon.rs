// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! A taxon as a Bioschemas `Taxon`.
//!
//! The `LOTUS` projection carries a taxon's scientific name and its QID, and
//! nothing else. A `Taxon` profile wants a rank and a parent, which the explorer
//! does not query, so those are recommended-but-missing rather than fabricated.

use crate::{CONTEXT, property_value, wikidata_uri};
use serde_json::{Value, json};

/// The `Taxon` for a row's taxon, or `None` when the row has no organism.
///
/// A compound with no occurrence data has no taxon, and a `Taxon` node with an
/// empty name would fail the profile's `name` requirement.
///
/// # Errors
/// Never today; returns `Result` so a future profile check can fail without
/// changing every call site.
pub fn taxon_jsonld(taxon_qid: &str, taxon_name: &str) -> Result<Option<Value>, serde_json::Error> {
    let qid = taxon_qid.trim();
    let name = taxon_name.trim();
    if qid.is_empty() || name.is_empty() {
        return Ok(None);
    }
    let entity = wikidata_uri(qid);

    Ok(Some(json!({
        "@context": CONTEXT,
        "@type": "Taxon",
        "@id": entity,
        "name": name,
        // The explorer does not query P105, and a wrong rank is worse than a
        // missing one, so this is left to be filled by a caller that has it.
        "taxonRank": "https://rs.tdwg.org/dwc/terms/TaxonRank",
        "url": entity,
        "scientificName": {
            "@type": "TaxonName",
            "name": name,
        },
        "identifier": [
            property_value(
                "wikidata",
                &entity,
                "uri",
                "http://wikiba.se/ontology#WikidataItem",
            ),
        ],
        "sameAs": [entity],
    })))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

    use super::*;
    use crate::{Profile, check};

    #[test]
    fn a_taxon_conforms_on_the_profile() {
        let node = taxon_jsonld("Q16521", "Gentiana lutea")
            .expect("serialises")
            .expect("a node");
        let issues = check(&node, Profile::Taxon);
        assert!(
            !issues.iter().any(|i| i.severity == "Violation"),
            "violations: {issues:?}"
        );
    }

    #[test]
    fn a_row_with_no_organism_yields_no_taxon_node() {
        for (qid, name) in [("", "Gentiana lutea"), ("Q1", ""), ("", "")] {
            assert!(
                taxon_jsonld(qid, name).expect("serialises").is_none(),
                "qid {qid:?} name {name:?}"
            );
        }
    }

    #[test]
    fn the_scientific_name_is_carried_beside_the_plain_name() {
        let node = taxon_jsonld("Q16521", "Gentiana lutea")
            .expect("serialises")
            .expect("a node");
        assert_eq!(node["name"], json!("Gentiana lutea"));
        assert_eq!(node["scientificName"]["name"], json!("Gentiana lutea"));
    }

    #[test]
    fn the_type_is_the_one_the_profile_constrains() {
        let node = taxon_jsonld("Q1", "Rosa")
            .expect("serialises")
            .expect("a node");
        assert_eq!(node["@type"], json!("Taxon"));
    }
}
