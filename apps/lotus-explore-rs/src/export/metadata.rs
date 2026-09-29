// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::filename::now_iso8601;
use super::filters::criteria_to_filters_value;
use crate::models::SearchCriteria;
use serde::Serialize;
use serde_json::{Map, Value, json};

pub const APP_VERSION: &str = "0.1.0";
pub const APP_NAME: &str = "LOTUS Explorer";
pub const APP_URL: &str = "https://github.com/lotusnprod/lotus-explore-rs";
pub const QLEVER_ENDPOINT: &str = "https://qlever.dev/api/wikidata";
pub const WDQS_ENDPOINT: &str = "https://query.wikidata.org/sparql";

/// Represents the SPARQL endpoint used for a query.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SparqlEndpoint {
    /// `QLever` endpoint (default/preferred)
    #[default]
    Qlever,
    /// Wikidata Query Service (fallback when `QLever` is unavailable)
    Wdqs,
}

impl std::fmt::Display for SparqlEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Qlever => write!(f, "QLever"),
            Self::Wdqs => write!(f, "Wikidata Query Service"),
        }
    }
}

impl SparqlEndpoint {
    pub fn info(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Qlever => (
                QLEVER_ENDPOINT,
                "QLever Wikidata",
                "Fast SPARQL endpoint for Wikidata (QLever)",
            ),
            Self::Wdqs => (
                WDQS_ENDPOINT,
                "Wikidata Query Service",
                "Wikidata Query Service (QLever fallback)",
            ),
        }
    }
}

#[derive(Serialize)]
struct Organization<'a> {
    #[serde(rename = "@type")]
    type_: &'a str,
    name: &'a str,
    url: &'a str,
}

#[derive(Serialize)]
struct Creator<'a> {
    #[serde(rename = "@type")]
    type_: &'a str,
    name: &'a str,
    version: &'a str,
    url: &'a str,
}

#[derive(Serialize)]
struct ScholarlyArticle<'a> {
    #[serde(rename = "@type")]
    type_: &'a str,
    name: &'a str,
    identifier: &'a str,
    url: &'a str,
}

#[derive(Serialize)]
struct DataDownload<'a> {
    #[serde(rename = "@type")]
    type_: &'a str,
    #[serde(rename = "encodingFormat")]
    encoding_format: &'a str,
    #[serde(rename = "contentUrl")]
    content_url: &'a str,
}

#[derive(Serialize)]
struct ChemicalSearchService<'a> {
    name: &'a str,
    provider: &'a str,
    endpoint: &'a str,
}

#[derive(Serialize)]
struct SparqlEndpointInfo {
    url: String,
    name: String,
    description: String,
}

#[derive(Serialize)]
struct Provenance {
    query_hash: HashInfo,
    result_hash: HashInfo,
    dataset_uri: String,
}

#[derive(Serialize)]
struct HashInfo {
    algorithm: &'static str,
    value: String,
}

#[derive(Serialize)]
struct DatasetMetadata {
    #[serde(rename = "@context")]
    context: &'static str,
    #[serde(rename = "@type")]
    type_: &'static str,
    name: String,
    description: String,
    version: &'static str,
    #[serde(rename = "dateCreated")]
    date_created: String,
    license: &'static str,
    creator: Creator<'static>,
    provider: Vec<Organization<'static>>,
    citation: Vec<ScholarlyArticle<'static>>,
    distribution: Vec<DataDownload<'static>>,
    #[serde(rename = "numberOfRecords", skip_serializing_if = "Option::is_none")]
    number_of_records: Option<usize>,
    #[serde(rename = "variablesMeasured")]
    variables_measured: Vec<&'static str>,
    search_parameters: Value,
    #[serde(
        rename = "chemical_search_service",
        skip_serializing_if = "Option::is_none"
    )]
    chemical_search_service: Option<ChemicalSearchService<'static>>,
    sparql_endpoint: SparqlEndpointInfo,
    provenance: Provenance,
}

pub struct MetadataInputs<'a> {
    pub criteria: &'a SearchCriteria,
    pub qid: Option<&'a str>,
    pub number_of_records_override: Option<usize>,
    pub query_hash: &'a str,
    pub result_hash: &'a str,
    /// The SPARQL endpoint used for this query (Qlever by default, WDQS on fallback)
    pub endpoint: SparqlEndpoint,
}

// The JSON body is assembled from many small, individually-necessary
// write! calls; splitting it across helpers would fracture one cohesive
// serialization unit more than the line count helps it.
#[allow(clippy::too_many_lines)]
// Called with an inline-constructed `MetadataInputs` at both call sites;
// taking it by reference would add a strict lifetime without benefit.
#[allow(clippy::needless_pass_by_value)]
pub fn build_metadata_json(inp: MetadataInputs<'_>) -> String {
    let filters = criteria_to_filters_value(inp.criteria);

    let effective_taxon = match (inp.qid, inp.criteria.taxon.trim()) {
        (Some("*"), _) | (None, "") => "all taxa".to_string(),
        (_, t) if !t.is_empty() => t.to_string(),
        (Some(q), _) => q.to_string(),
        _ => "all taxa".to_string(),
    };

    let chem = filters.get("chemical_structure").cloned();
    let (dataset_name, description) = chem.as_ref().map_or_else(
        || {
            (
                format!("LOTUS Data — {effective_taxon}"),
                format!(
                    "Chemical compounds from {effective_taxon}. Retrieved via LOTUS  Explorer."
                ),
            )
        },
        |c| {
            let st = c
                .get("search_type")
                .and_then(|v| v.as_str())
                .unwrap_or("substructure");
            (
                format!(
                    "LOTUS Data — {} search in {effective_taxon}",
                    title_case(st)
                ),
                format!(
                    "Chemical compounds from {effective_taxon}. Retrieved via LOTUS \
                     Knowledge Search with {st} chemical search (SACHEM/IDSM)."
                ),
            )
        },
    );

    let mut providers = vec![
        Organization {
            type_: "Organization",
            name: "LOTUS Initiative",
            url: "https://www.wikidata.org/wiki/Q104225190",
        },
        Organization {
            type_: "Organization",
            name: "Wikidata",
            url: "http://www.wikidata.org/",
        },
    ];
    if chem.is_some() {
        providers.push(Organization {
            type_: "Organization",
            name: "IDSM",
            url: "https://idsm.elixir-czech.cz/",
        });
    }

    let mut search_params = Map::new();
    search_params.insert("taxon".into(), Value::String(effective_taxon));
    search_params.insert(
        "taxon_qid".into(),
        match inp.qid {
            Some(q) if q != "*" => Value::String(q.to_string()),
            _ => Value::Null,
        },
    );

    if let Some(c) = chem.as_ref() {
        let smiles_str = c
            .get("smiles")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let multiline = smiles_str.contains('\n') || smiles_str.contains('\r');

        let mut sq = Map::new();
        sq.insert("param_key".into(), "structure".into());
        sq.insert("legacy_param_key".into(), "smiles".into());
        sq.insert(
            "search_type".into(),
            c.get("search_type")
                .cloned()
                .unwrap_or_else(|| json!("substructure")),
        );
        sq.insert(
            "input_format".into(),
            Value::String(if multiline {
                "molfile".into()
            } else {
                "smiles".into()
            }),
        );
        if let Some(t) = c.get("similarity_threshold").cloned() {
            sq.insert("similarity_threshold".into(), t);
        }
        if multiline {
            sq.insert(
                "query_preview".into(),
                Value::String(smiles_str.chars().take(500).collect()),
            );
            sq.insert(
                "query_length".into(),
                Value::Number(smiles_str.len().into()),
            );
        } else {
            sq.insert("query_text".into(), Value::String(smiles_str));
        }
        search_params.insert("structure_query".into(), Value::Object(sq));
    }

    if let Some(obj) = filters.as_object()
        && !obj.is_empty()
    {
        search_params.insert("filters".into(), filters.clone());
    }

    let chemical_search_service = chem.is_some().then_some(ChemicalSearchService {
        name: "SACHEM",
        provider: "IDSM",
        endpoint: "https://idsm.elixir-czech.cz/sparql/endpoint/",
    });

    let dataset = DatasetMetadata {
        context: "https://schema.org/",
        type_: "Dataset",
        name: dataset_name,
        description,
        version: APP_VERSION,
        date_created: now_iso8601(),
        license: "https://creativecommons.org/publicdomain/zero/1.0/",
        creator: Creator {
            type_: "SoftwareApplication",
            name: APP_NAME,
            version: APP_VERSION,
            url: APP_URL,
        },
        provider: providers,
        citation: vec![ScholarlyArticle {
            type_: "ScholarlyArticle",
            name: "The LOTUS initiative for open knowledge management in natural products research",
            identifier: "https://doi.org/10.7554/eLife.70780",
            url: "https://doi.org/10.7554/eLife.70780",
        }],
        distribution: vec![
            DataDownload {
                type_: "DataDownload",
                encoding_format: "text/csv",
                content_url: "data:text/csv",
            },
            DataDownload {
                type_: "DataDownload",
                encoding_format: "application/sparql-results+json",
                content_url: "data:application/sparql-results+json",
            },
            DataDownload {
                type_: "DataDownload",
                encoding_format: "text/turtle",
                content_url: "data:text/turtle",
            },
        ],
        number_of_records: inp.number_of_records_override,
        variables_measured: vec![
            "compound_name",
            "compound_smiles",
            "compound_inchikey",
            "compound_mass",
            "molecular_formula",
            "taxon_name",
            "reference_title",
            "reference_doi",
            "reference_date",
            "compound_qid",
            "taxon_qid",
            "reference_qid",
        ],
        search_parameters: Value::Object(search_params),
        chemical_search_service,
        sparql_endpoint: {
            let (url, name, description) = inp.endpoint.info();
            SparqlEndpointInfo {
                url: url.to_string(),
                name: name.to_string(),
                description: description.to_string(),
            }
        },
        provenance: Provenance {
            query_hash: HashInfo {
                algorithm: "SHA-256",
                value: inp.query_hash.to_string(),
            },
            result_hash: HashInfo {
                algorithm: "SHA-256",
                value: inp.result_hash.to_string(),
            },
            dataset_uri: format!("urn:hash:sha256:{}", inp.result_hash),
        },
    };

    serde_json::to_string_pretty(&dataset).unwrap_or_default()
}

fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    chars
        .next()
        .map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_json_contains_schema_dataset() {
        let criteria = SearchCriteria::up_to_year(crate::models::current_year());
        let body = build_metadata_json(MetadataInputs {
            criteria: &criteria,
            qid: Some("Q42"),
            number_of_records_override: Some(1),
            query_hash: "abc",
            result_hash: "def",
            endpoint: SparqlEndpoint::Qlever,
        });
        assert!(body.contains("\"@type\": \"Dataset\""));
        assert!(body.contains("\"query_hash\""));
    }

    #[test]
    fn metadata_json_shows_wdqs_when_fallback_used() {
        let criteria = SearchCriteria::up_to_year(crate::models::current_year());
        let body = build_metadata_json(MetadataInputs {
            criteria: &criteria,
            qid: Some("Q42"),
            number_of_records_override: Some(1),
            query_hash: "abc",
            result_hash: "def",
            endpoint: SparqlEndpoint::Wdqs,
        });
        assert!(body.contains(&format!("\"url\": \"{WDQS_ENDPOINT}\"")));
        assert!(body.contains("Wikidata Query Service"));
        assert!(body.contains("QLever fallback"));
    }
}
