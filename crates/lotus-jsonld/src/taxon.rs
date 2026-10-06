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
#[path = "taxon/tests.rs"]
mod tests;
