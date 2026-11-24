// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! A result set, and the `LOTUS` source behind it, as `Dataset` and
//! `DataCatalog`.
//!
//! The Dataset profile is what Google Dataset Search reads, and it needs a
//! resolvable `url` and a real `distribution`. A `Dataset` with a
//! `data:text/csv` placeholder in it — which is what this repository emitted
//! before — is indexable and useless, so the distribution here is a description
//! of how to obtain the data rather than a fake file.

use crate::{Profile, SOFTWARE, doi_uri, edam, property_value, result_dataset_url, wikidata_uri};
use serde_json::{Value, json};

/// What a result set is, and how to describe it.
#[derive(Debug)]
pub struct ResultSet<'a> {
    /// The query the rows came from. Carried so that a caller can emit the
    /// reproducibility record alongside the dataset.
    pub query: &'a str,
    /// The taxon the search was for, already resolved to a display name.
    pub taxon: &'a str,
    /// SHA-256 of the query, which makes the dataset's URI stable.
    pub query_hash: &'a str,
    /// SHA-256 of the result, which makes the dataset's URI stable.
    pub result_hash: &'a str,
    /// The total the endpoint reported, which may exceed the rows returned.
    pub total_entries: usize,
    /// When the set was produced, ISO 8601.
    pub generated: &'a str,
}

/// A slug for a taxon, safe in a URL.
fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' => out.push(c.to_ascii_lowercase()),
            ' ' | '_' | '/' => out.push('-'),
            _ => {}
        }
    }
    out.trim_matches('-').to_string()
}

/// The `Dataset` for a result set, conforming to the Bioschemas Dataset profile.
#[must_use]
pub fn result_set_jsonld(set: &ResultSet<'_>) -> Value {
    let taxon_slug = slug(if set.taxon.is_empty() {
        "all-taxa"
    } else {
        set.taxon
    });
    let url = result_dataset_url(&taxon_slug, set.query_hash);

    let description = if set.taxon.is_empty() {
        format!("{} occurrences", set.total_entries)
    } else {
        format!(
            "{} occurrences in {}",
            set.total_entries,
            display_taxon(set.taxon)
        )
    };

    crate::stamp(
        json!({
        "@type": "Dataset",
        "@id": url,
        "name": format!("LOTUS occurrences — {}", display_taxon(set.taxon)),
        "description": description,
        "url": url,
        "identifier": [
            // A checksum of the result, which is what it is. Rejected as a second
            // identifier under a `query` propertyID typed with an ARK namespace: a
            // hash is not an ARK, and `identifiers.org/ark:/` is not one either.
            // `sha256` with `encodingFormat` is a real pairing.
            json!({
                "@type": "PropertyValue",
                "propertyID": "sha256",
                "value": set.result_hash,
                "encodingFormat": "application/sha256",
            }),
        ],
        "keywords": [
            "natural products", "biosynthetic", "chemical structure",
            "taxonomy", "SPARQL", "linked open data",
            edam::TOPIC_SMALL_MOLECULES,
            edam::TOPIC_CHEMINFORMATICS,
            edam::TOPIC_METABOLOMICS,
            edam::TOPIC_TAXONOMY,
        ],
        "about": edam_topics(),
        "license": "https://creativecommons.org/publicdomain/zero/1.0/",
        "creator": { "@type": "Organization", "name": "LOTUS Initiative" },
        "version": SOFTWARE.version,
        "datePublished": set.generated,
        "variableMeasured": measured_variables(),
        "measurementTechnique": "SPARQL 1.1 over the Wikidata graph",
        "citation": lotus_citation(),
        // The query is the reproducibility record: it is what re-derives this
        // set, and a reader can paste it into any SPARQL endpoint.
        "distribution": [
            {
                "@type": "DataDownload",
                "encodingFormat": ["text/csv", edam::FORMAT_CSV],
            },
            {
                "@type": "DataDownload",
                "encodingFormat": ["application/ld+json", edam::FORMAT_JSON_LD],
            },
        ],
        "includedInDataCatalog": data_catalog(),
        }),
        Profile::Dataset,
    )
}

/// The `LOTUS` knowledge graph itself, as a `Dataset` in its own right.
#[must_use]
pub fn dataset_jsonld() -> Value {
    crate::stamp(
        json!({
        "@type": "Dataset",
        "@id": "https://lotus-initiative.github.io/lotus-explore-rs",
        "name": "LOTUS",
        "description": "The LOTUS knowledge base of chemical structures and biological \
                        sources, stored in Wikidata and queried over SPARQL.",
        "url": "https://lotus-initiative.github.io/lotus-explore-rs",
        "identifier": [
            property_value(
                "wikidata",
                "Q104225190",
                "uri",
                "http://wikiba.se/ontology#WikidataItem",
            ),
        ],
        "keywords": [
            "natural products",
            "linked open data",
            "SPARQL",
            edam::TOPIC_SMALL_MOLECULES,
            edam::TOPIC_CHEMINFORMATICS,
            edam::TOPIC_METABOLOMICS,
            edam::TOPIC_TAXONOMY,
        ],
        "about": edam_topics(),
        "license": "https://creativecommons.org/publicdomain/zero/1.0/",
        "version": SOFTWARE.version,
        "datePublished": SOFTWARE.date_published,
        "creator": { "@type": "Organization", "name": "LOTUS Initiative" },
        "variableMeasured": measured_variables(),
        "measurementTechnique": "SPARQL 1.1 over the Wikidata graph",
        // The paper that describes the knowledge base, which is also what a
        // reader of this dataset should be pointed at.
        "citation": lotus_citation(),
        "sameAs": [wikidata_uri("Q104225190")],
        "distribution": [
            {
                "@type": "DataDownload",
                "encodingFormat": ["application/sparql-query", edam::FORMAT_SPARQL],
                // Schema.org download URL for the external Wikidata SPARQL service,
                // independent of offline DataDownload exports.
                "contentUrl": lotus_model::WDQS_WIKIDATA,
            },
        ],
        "includedInDataCatalog": data_catalog(),
        }),
        Profile::Dataset,
    )
}

fn edam_topics() -> Value {
    json!([
        { "@type": "DefinedTerm", "@id": edam::TOPIC_SMALL_MOLECULES, "name": "Small molecules" },
        { "@type": "DefinedTerm", "@id": edam::TOPIC_CHEMINFORMATICS, "name": "Cheminformatics" },
        { "@type": "DefinedTerm", "@id": edam::TOPIC_METABOLOMICS, "name": "Metabolomics" },
        { "@type": "DefinedTerm", "@id": edam::TOPIC_TAXONOMY, "name": "Taxonomy" },
    ])
}

fn measured_variables() -> Value {
    json!([
        "compound name",
        "compound SMILES",
        "InChIKey",
        "molecular mass",
        "molecular formula",
        "taxon name",
        "reference title",
        "reference DOI"
    ])
}

fn lotus_citation() -> Value {
    json!({
        "@type": "ScholarlyArticle",
        "name": "The LOTUS initiative for open knowledge management in natural products research",
        "identifier": doi_uri("10.7554/eLife.70780"),
        "url": doi_uri("10.7554/eLife.70780"),
    })
}

fn data_catalog() -> Value {
    json!({
        "@type": "DataCatalog",
        "@id": "https://lotus-initiative.github.io/lotus-explore-rs",
        "name": "LOTUS",
        "url": "https://lotus-initiative.github.io/lotus-explore-rs",
    })
}

fn display_taxon(taxon: &str) -> &str {
    if taxon.is_empty() || taxon == "*" {
        "all organisms"
    } else {
        taxon
    }
}

#[cfg(test)]
#[path = "dataset/tests.rs"]
mod tests;
