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

use crate::{Profile, SOFTWARE, doi_uri, property_value, result_dataset_url, wikidata_uri};
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

    let mut node = json!({
        "@type": "Dataset",
        "@id": url,
        "name": format!("LOTUS occurrences — {}", display_taxon(set.taxon)),
        "description": format!(
            "Chemical compounds reported in {} in the LOTUS knowledge graph, \
             retrieved by SPARQL from Wikidata and served by the LOTUS Explorer.",
            display_taxon(set.taxon)
        ),
        "url": url,
        "identifier": [
            property_value(
                "query",
                set.query_hash,
                "string",
                "https://identifiers.org/ark:/"
            ),
            json!({
                "@type": "PropertyValue",
                "propertyID": "sha256",
                "value": set.result_hash,
                "encodingFormat": "application/sha256",
            }),
        ],
        "keywords": [
            "natural products", "biosynthetic", "chemical structure",
            "taxonomy", "SPARQL", "linked open data"
        ],
        "license": "https://creativecommons.org/publicdomain/zero/1.0/",
        "creator": { "@type": "Organization", "name": "LOTUS Initiative" },
        "version": SOFTWARE.version,
        "datePublished": set.generated,
        "variableMeasured": [
            "compound name", "compound SMILES", "InChIKey", "molecular mass",
            "molecular formula", "taxon name", "reference title", "reference DOI"
        ],
        "measurementTechnique": "SPARQL 1.1 over the Wikidata graph",
        "citation": {
            "@type": "ScholarlyArticle",
            "name": "The LOTUS initiative for open knowledge management in natural products research",
            "identifier": doi_uri("10.7554/eLife.70780"),
            "url": doi_uri("10.7554/eLife.70780"),
        },
        // The query is the reproducibility record: it is what re-derives this
        // set, and a reader can paste it into any SPARQL endpoint.
        "distribution": [
            {
                "@type": "DataDownload",
                "encodingFormat": "text/csv",
                "contentUrl": format!("{url}?format=csv&query={}", set.query_hash),
            },
            {
                "@type": "DataDownload",
                "encodingFormat": "application/ld+json",
                "contentUrl": format!("{url}?format=jsonld&query={}", set.query_hash),
            },
        ],
        "includedInDataCatalog": {
            "@type": "DataCatalog",
            "@id": "https://lotus.nprod.net/lotus-explore-rs",
            "name": "LOTUS",
            "url": "https://lotus.nprod.net/lotus-explore-rs",
        },
    });

    if let Some(object) = node.as_object_mut() {
        object.insert(
            "description".into(),
            json!(format!(
                "{} occurrences{}",
                set.total_entries,
                if set.taxon.is_empty() {
                    String::new()
                } else {
                    format!(" in {}", display_taxon(set.taxon))
                }
            )),
        );
    }
    crate::stamp(node, Profile::Dataset)
}

/// The `LOTUS` knowledge graph itself, as a `Dataset` in its own right.
#[must_use]
pub fn dataset_jsonld() -> Value {
    crate::stamp(
        json!({
        "@type": "Dataset",
        "@id": "https://lotus.nprod.net/lotus-explore-rs",
        "name": "LOTUS",
        "description": "The LOTUS knowledge base of chemical structures and biological \
                        sources, stored in Wikidata and queried over SPARQL.",
        "url": "https://lotus.nprod.net/lotus-explore-rs",
        "identifier": [
            property_value(
                "wikidata",
                "Q104225190",
                "uri",
                "http://wikiba.se/ontology#WikidataItem",
            ),
        ],
        "keywords": ["natural products", "linked open data", "SPARQL"],
        "license": "https://creativecommons.org/publicdomain/zero/1.0/",
        "version": SOFTWARE.version,
        "creator": { "@type": "Organization", "name": "LOTUS Initiative" },
        // The paper that describes the knowledge base, which is also what a
        // reader of this dataset should be pointed at.
        "citation": {
            "@type": "ScholarlyArticle",
            "name": "The LOTUS initiative for open knowledge management in natural products research",
            "identifier": doi_uri("10.7554/eLife.70780"),
            "url": doi_uri("10.7554/eLife.70780"),
        },
        "sameAs": [wikidata_uri("Q104225190")],
        "distribution": [
            {
                "@type": "DataDownload",
                "encodingFormat": "application/sparql-query",
                "contentUrl": "https://query.wikidata.org/sparql",
            },
        ],
        "includedInDataCatalog": {
            "@type": "DataCatalog",
            "@id": "https://lotus.nprod.net/lotus-explore-rs",
            "name": "LOTUS",
            "url": "https://lotus.nprod.net/lotus-explore-rs",
        },
        }),
        Profile::Dataset,
    )
}

fn display_taxon(taxon: &str) -> String {
    if taxon.is_empty() || taxon == "*" {
        "all organisms".to_string()
    } else {
        taxon.to_string()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

    use super::*;
    use crate::Profile;
    use crate::profile::check;
    use lotus_model::CompoundEntry;
    use std::sync::Arc;

    fn rows() -> Vec<CompoundEntry> {
        vec![CompoundEntry {
            compound_qid: Arc::from("Q1"),
            name: Arc::from("Quercetin"),
            ..CompoundEntry::default()
        }]
    }

    fn set<'a>(_rows: &'a [CompoundEntry], query: &'a str) -> ResultSet<'a> {
        ResultSet {
            query,
            taxon: "Gentiana lutea",
            query_hash: "abc",
            result_hash: "def",
            total_entries: 1234,
            generated: "2026-01-01T00:00:00Z",
        }
    }

    #[test]
    fn a_result_set_conforms_on_the_dataset_profile() {
        let rows = rows();
        let node = result_set_jsonld(&set(&rows, "SELECT ?s WHERE {}"));
        let issues = check(&node, Profile::Dataset);
        assert!(
            !issues.iter().any(|i| i.severity == "Violation"),
            "violations: {issues:?}"
        );
    }

    #[test]
    fn the_lotus_source_conforms_on_the_dataset_profile() {
        let issues = check(&dataset_jsonld(), Profile::Dataset);
        assert!(
            !issues.iter().any(|i| i.severity == "Violation"),
            "violations: {issues:?}"
        );
    }

    #[test]
    fn a_distribution_points_at_something_that_can_be_fetched() {
        // The point of the change: no `data:` placeholder, which is indexable and
        // cannot be downloaded.
        let rows = rows();
        let node = result_set_jsonld(&set(&rows, "SELECT ?s WHERE {}"));
        for download in node["distribution"].as_array().expect("an array") {
            let url = download["contentUrl"].as_str().expect("a url");
            assert!(url.starts_with("https://"), "got {url}");
            assert!(!url.starts_with("data:"), "a data: URL cannot be fetched");
        }
    }

    #[test]
    fn the_url_is_stable_for_the_same_query_hash() {
        let rows = rows();
        let first = result_set_jsonld(&set(&rows, "SELECT ?s WHERE {}"));
        let second = result_set_jsonld(&set(&rows, "a different query"));
        assert_eq!(first["@id"], second["@id"], "the hash, not the query text");
    }

    #[test]
    fn a_taxon_name_becomes_a_url_slug() {
        let rows = rows();
        let node = result_set_jsonld(&set(&rows, "SELECT ?s WHERE {}"));
        assert_eq!(
            node["@id"],
            json!("https://lotus.nprod.net/lotus-explore-rs/dataset/gentiana-lutea/abc")
        );
    }

    #[test]
    fn an_empty_taxon_reads_as_all_organisms() {
        let rows = rows();
        let mut s = set(&rows, "SELECT ?s WHERE {}");
        s.taxon = "";
        let node = result_set_jsonld(&s);
        assert!(
            node["name"]
                .as_str()
                .expect("a name")
                .contains("all organisms")
        );
        assert_eq!(
            node["@id"],
            json!("https://lotus.nprod.net/lotus-explore-rs/dataset/all-taxa/abc")
        );
    }

    #[test]
    fn the_reported_total_is_used_rather_than_the_rows_returned() {
        // The rows are a page; the total describes the set.
        let rows = rows();
        let node = result_set_jsonld(&set(&rows, "SELECT ?s WHERE {}"));
        assert!(
            node["description"]
                .as_str()
                .expect("a description")
                .contains("1234"),
            "got: {}",
            node["description"]
        );
    }

    #[test]
    fn the_citation_is_the_lotus_paper() {
        let node = dataset_jsonld();
        assert_eq!(
            node["citation"]["identifier"],
            json!("https://doi.org/10.7554/eLife.70780")
        );
    }

    #[test]
    fn an_absent_taxon_is_shown_as_all_organisms() {
        // Two spellings of "no taxon": an empty cell and the `*` the SPARQL uses
        // for a wildcard. Both mean the same thing to a reader, so both are
        // written the same way.
        for wildcard in ["", "*"] {
            assert_eq!(display_taxon(wildcard), "all organisms", "{wildcard:?}");
        }
        assert_eq!(
            display_taxon("Q16521"),
            "Q16521",
            "a real taxon is shown as itself"
        );
    }
}
