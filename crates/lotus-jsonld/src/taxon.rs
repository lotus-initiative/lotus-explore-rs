// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! A taxon as a Bioschemas `Taxon`.
//!
//! The `LOTUS` projection carries a taxon's scientific name and its QID, and
//! nothing else. The `Taxon` profile also wants a rank, which the explorer does
//! not query, so [`taxon_jsonld`] takes one and omits the property when it is
//! not given. It used to emit a placeholder, which is worse than nothing: a
//! consumer reading `taxonRank` would take it for a value the data does not
//! have.

use crate::{property_value, wikidata_uri};
use serde_json::{Value, json};

/// The `Taxon` for a row's taxon, or `None` when the row names none.
///
/// `None` when the row names no taxon: a `Taxon` node with an empty name fails
/// the profile's `name` requirement.
///
/// `rank` is the taxon rank as a schema.org term -- `"http://schema.org/Species"`,
/// say -- passed rather than looked up because the explorer does not query P105,
/// and a rank invented from a scientific name is a guess presented as a fact.
///
/// # Errors
/// Never today; returns `Result` so a future profile check can fail without
/// changing every call site.
pub fn taxon_jsonld(
    taxon_qid: &str,
    taxon_name: &str,
    rank: Option<&str>,
) -> Result<Option<Value>, serde_json::Error> {
    let qid = taxon_qid.trim();
    let name = taxon_name.trim();
    if qid.is_empty() || name.is_empty() {
        return Ok(None);
    }
    let entity = wikidata_uri(qid);

    let mut node = json!({
        "@type": "Taxon",
        "@id": entity,
        "name": name,
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
    });

    // Only present when it is known. A missing recommended property is a
    // warning from a validator; a wrong one is a fact the data does not have.
    if let Some(object) = node.as_object_mut()
        && let Some(rank) = rank.map(str::trim).filter(|r| !r.is_empty())
    {
        object.insert("taxonRank".into(), rank.into());
    }

    Ok(Some(crate::stamp(node, crate::Profile::Taxon)))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

    use super::*;
    use crate::{Profile, check};

    fn node(rank: Option<&str>) -> Value {
        taxon_jsonld("Q16521", "Gentiana lutea", rank)
            .expect("serialises")
            .expect("a node")
    }

    #[test]
    fn a_taxon_conforms_on_the_profile() {
        for rank in [None, Some("http://schema.org/Species")] {
            let issues = check(&node(rank), Profile::Taxon);
            assert!(
                !issues.iter().any(|i| i.severity == "Violation"),
                "rank {rank:?} violations: {issues:?}"
            );
        }
    }

    #[test]
    fn a_row_with_no_organism_yields_no_taxon_node() {
        for (qid, name) in [("", "Gentiana lutea"), ("Q1", ""), ("", "")] {
            assert!(
                taxon_jsonld(qid, name, None).expect("serialises").is_none(),
                "qid {qid:?} name {name:?}"
            );
        }
    }

    #[test]
    fn the_scientific_name_is_carried_beside_the_plain_name() {
        let node = node(None);
        assert_eq!(node["name"], json!("Gentiana lutea"));
        assert_eq!(node["scientificName"]["name"], json!("Gentiana lutea"));
    }

    #[test]
    fn the_type_is_the_one_the_profile_constrains() {
        assert_eq!(node(None)["@type"], json!("Taxon"));
    }

    #[test]
    fn an_unknown_rank_is_omitted_rather_than_guessed() {
        // This is the behaviour that changed. The old code emitted
        // `https://rs.tdwg.org/dwc/terms/TaxonRank`, which is the *class* of
        // thing a rank is, not the rank of anything.
        let node = node(None);
        assert!(
            node.get("taxonRank").is_none(),
            "a rank that was not given should not appear: {node}"
        );
    }

    #[test]
    fn a_blank_rank_counts_as_unknown() {
        for blank in ["", "   ", "\t"] {
            assert!(
                node(Some(blank)).get("taxonRank").is_none(),
                "blank rank {blank:?}"
            );
        }
    }

    #[test]
    fn a_known_rank_is_carried_through() {
        let node = node(Some("http://schema.org/Species"));
        assert_eq!(node["taxonRank"], json!("http://schema.org/Species"));
    }
}
