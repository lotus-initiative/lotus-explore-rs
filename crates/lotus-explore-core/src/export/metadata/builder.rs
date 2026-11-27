// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Assembly helpers for Schema.org / `BioSchemas` export metadata JSON.

use super::super::filters::criteria_to_filters_value;
use super::schema::{
    ChemicalSearchService, Creator, DataDownload, DatasetMetadata, HashInfo, MEASURED_VARIABLES,
    Organization, Provenance, ScholarlyArticle, SparqlEndpointInfo,
};
use super::{APP_NAME, APP_URL, APP_VERSION, MetadataInputs};
use lotus_jsonld::{
    CC0_LICENSE_URL, LOTUS_PAPER_TITLE, LOTUS_PAPER_URL, LOTUS_WIKIDATA_URL, Profile, SOFTWARE,
    edam, result_dataset_url,
};
use serde_json::{Map, Value, json};

// Called with an inline-constructed `MetadataInputs` at both call sites;
// taking it by reference would add a strict lifetime without benefit.
#[allow(
    clippy::needless_pass_by_value,
    reason = "callers pass inline MetadataInputs literals by value"
)]
/// The metadata document for one export, as a JSON string.
#[must_use]
pub fn build_metadata_json(inp: MetadataInputs<'_>) -> String {
    let filters = criteria_to_filters_value(inp.criteria, inp.year);
    let effective_taxon = resolve_effective_taxon(&inp);
    let chem = filters.get("chemical_structure").cloned();
    let sachem = uses_sachem(inp.criteria);
    let (dataset_name, description) =
        build_title_and_description(&effective_taxon, chem.as_ref(), sachem);
    let providers = build_providers(sachem);
    let dataset_url = result_dataset_url(
        &lotus_model::percent_encode(&effective_taxon),
        inp.query_hash,
    );
    let about_terms = build_about_terms(inp.qid, &effective_taxon);
    let search_params = build_search_params(
        inp.criteria,
        &effective_taxon,
        inp.qid,
        chem.as_ref(),
        &filters,
    );
    let chemical_search_service = sachem.then_some(ChemicalSearchService {
        name: "SACHEM",
        provider: "IDSM",
        endpoint: lotus_model::IDSM_SACHEM_ENDPOINT,
    });

    let dataset = DatasetMetadata {
        context: lotus_jsonld::jsonld_context(),
        type_: Profile::Dataset.rdf_type(),
        id: dataset_url.clone(),
        conforms_to: Profile::Dataset.id(),
        name: dataset_name,
        description,
        url: dataset_url,
        identifier: build_identifier(inp.result_hash),
        keywords: build_keywords(),
        about: Value::Array(about_terms),
        version: APP_VERSION,
        date_created: inp.date_created.to_string(),
        date_published: inp.date_created.to_string(),
        license: CC0_LICENSE_URL,
        creator: Creator {
            type_: "SoftwareApplication",
            name: APP_NAME,
            version: APP_VERSION,
            url: APP_URL,
        },
        provider: providers,
        citation: vec![ScholarlyArticle {
            type_: "ScholarlyArticle",
            name: LOTUS_PAPER_TITLE,
            identifier: LOTUS_PAPER_URL,
            url: LOTUS_PAPER_URL,
        }],
        distribution: build_distributions(),
        included_in_data_catalog: json!({
            "@type": "DataCatalog",
            "@id": SOFTWARE.url,
            "name": "LOTUS",
            "url": SOFTWARE.url,
        }),
        measurement_technique: "SPARQL 1.1 over the Wikidata graph",
        number_of_records: inp.number_of_records_override,
        variable_measured: MEASURED_VARIABLES,
        search_parameters: Value::Object(search_params),
        chemical_search_service,
        sparql_endpoint: endpoint_info(inp.endpoint),
        sparql_query: inp
            .sparql_query
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .map(ToOwned::to_owned),
        provenance: build_provenance(inp.query_hash, inp.result_hash),
    };

    serde_json::to_string_pretty(&dataset).unwrap_or_default()
}

fn resolve_effective_taxon(inp: &MetadataInputs<'_>) -> String {
    match (inp.qid, inp.criteria.taxon.trim()) {
        (Some("*"), _) | (None, "") => "all taxa".to_string(),
        (_, t) if !t.is_empty() => t.to_string(),
        (Some(q), _) => q.to_string(),
        _ => "all taxa".to_string(),
    }
}

fn uses_sachem(criteria: &lotus_model::SearchCriteria) -> bool {
    let structure = criteria.structure.trim();
    if structure.is_empty() {
        return false;
    }
    if criteria.structure_search != lotus_model::SmilesSearchType::Exact {
        return true;
    }
    !lotus_model::names_a_compound(structure)
}

fn build_title_and_description(
    effective_taxon: &str,
    chem: Option<&Value>,
    sachem: bool,
) -> (String, String) {
    chem.map_or_else(
        || {
            (
                format!("LOTUS Data — {effective_taxon}"),
                format!("Chemical compounds from {effective_taxon}. Retrieved via LOTUS Explorer."),
            )
        },
        |c| {
            let st = c
                .get("search_type")
                .and_then(|v| v.as_str())
                .unwrap_or("substructure");
            let backend = if sachem {
                "chemical search (SACHEM/IDSM)"
            } else {
                "compound lookup (Wikidata)"
            };
            (
                format!(
                    "LOTUS Data — {} search in {effective_taxon}",
                    title_case(st)
                ),
                format!(
                    "Chemical compounds from {effective_taxon}. Retrieved via LOTUS \
                     Explorer with {st} {backend}."
                ),
            )
        },
    )
}

fn build_providers(has_chem: bool) -> Vec<Organization<'static>> {
    let mut providers = vec![
        Organization {
            type_: "Organization",
            name: "LOTUS Initiative",
            url: LOTUS_WIKIDATA_URL,
        },
        Organization {
            type_: "Organization",
            name: "Wikidata",
            url: "https://www.wikidata.org/",
        },
    ];
    if has_chem {
        providers.push(Organization {
            type_: "Organization",
            name: "IDSM",
            url: "https://idsm.elixir-czech.cz/",
        });
    }
    providers
}

fn build_about_terms(qid: Option<&str>, effective_taxon: &str) -> Vec<Value> {
    let mut about_terms = vec![
        json!({ "@type": "DefinedTerm", "@id": edam::TOPIC_SMALL_MOLECULES, "name": "Small molecules" }),
        json!({ "@type": "DefinedTerm", "@id": edam::TOPIC_CHEMINFORMATICS, "name": "Cheminformatics" }),
        json!({ "@type": "DefinedTerm", "@id": edam::TOPIC_METABOLOMICS, "name": "Metabolomics" }),
        json!({ "@type": "DefinedTerm", "@id": edam::TOPIC_TAXONOMY, "name": "Taxonomy" }),
    ];
    if let Some(q) = qid
        && q != "*"
        && let Ok(Some(taxon_node)) = lotus_jsonld::taxon_jsonld(q, effective_taxon, None)
    {
        about_terms.push(taxon_node);
    }
    about_terms
}

fn build_search_params(
    criteria: &lotus_model::SearchCriteria,
    effective_taxon: &str,
    qid: Option<&str>,
    chem: Option<&Value>,
    filters: &Value,
) -> Map<String, Value> {
    let mut search_params = Map::new();
    search_params.insert("taxon".into(), Value::String(effective_taxon.to_string()));
    search_params.insert(
        "taxon_qid".into(),
        match qid {
            Some(q) if q != "*" => Value::String(q.to_string()),
            _ => Value::Null,
        },
    );
    let reference = criteria.reference.trim();
    if !reference.is_empty() {
        search_params.insert("reference".into(), Value::String(reference.to_string()));
    }
    if criteria.taxon_names != lotus_model::TaxonNomenclature::default() {
        search_params.insert(
            "taxon_nomenclature".into(),
            json!({
                "accepted_synonyms": criteria.taxon_names.accepted_synonyms,
                "basionyms": criteria.taxon_names.basionyms,
                "protonyms": criteria.taxon_names.protonyms,
                "replacements": criteria.taxon_names.replacements,
            }),
        );
    }

    if let Some(c) = chem {
        search_params.insert(
            "structure_query".into(),
            Value::Object(build_structure_query(c)),
        );
    }

    if let Some(obj) = filters.as_object()
        && !obj.is_empty()
    {
        search_params.insert("filters".into(), filters.clone());
    }

    search_params
}

fn build_structure_query(c: &Value) -> Map<String, Value> {
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
    sq
}

fn build_identifier(result_hash: &str) -> Value {
    json!([
        {
            "@type": "PropertyValue",
            "propertyID": "sha256",
            "value": result_hash,
            "encodingFormat": "application/sha256",
        }
    ])
}

fn build_keywords() -> Vec<&'static str> {
    vec![
        "natural products",
        "chemical structure",
        "taxonomy",
        "SPARQL",
        "linked open data",
        edam::TOPIC_SMALL_MOLECULES,
        edam::TOPIC_CHEMINFORMATICS,
        edam::TOPIC_METABOLOMICS,
        edam::TOPIC_TAXONOMY,
    ]
}

fn build_distributions() -> Vec<DataDownload> {
    vec![
        DataDownload {
            type_: "DataDownload",
            encoding_format: ["text/csv", edam::FORMAT_CSV],
        },
        DataDownload {
            type_: "DataDownload",
            encoding_format: ["application/sparql-results+json", edam::FORMAT_JSON],
        },
        DataDownload {
            type_: "DataDownload",
            encoding_format: ["text/turtle", edam::FORMAT_TURTLE],
        },
    ]
}

fn endpoint_info(endpoint: lotus_model::SparqlEndpoint) -> SparqlEndpointInfo {
    let (url, name, description) = endpoint.info();
    SparqlEndpointInfo {
        url: url.to_string(),
        name: name.to_string(),
        description: description.to_string(),
    }
}

fn build_provenance(query_hash: &str, result_hash: &str) -> Provenance {
    Provenance {
        query_hash: HashInfo {
            algorithm: "SHA-256",
            value: query_hash.to_string(),
        },
        result_hash: HashInfo {
            algorithm: "SHA-256",
            value: result_hash.to_string(),
        },
        dataset_uri: format!("urn:hash:sha256:{result_hash}"),
    }
}

fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    chars
        .next()
        .map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
}
