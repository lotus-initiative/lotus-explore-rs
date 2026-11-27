// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `metadata`, in their own file.

#![allow(clippy::expect_used)]

use super::*;
use lotus_model::SparqlEndpoint;

/// The year the tests pin, so "is this year bound in the future?" has one
/// answer rather than the answer the day the suite runs.
const YEAR: u16 = 2026;

/// The instant the tests pin, so the document they assert is the whole one.
const NOW: &str = "2026-03-04T05:06:07Z";

#[test]
fn metadata_json_contains_schema_dataset() {
    let criteria = SearchCriteria::up_to_year(YEAR);
    let body = build_metadata_json(MetadataInputs {
        criteria: &criteria,
        qid: Some("Q42"),
        number_of_records_override: Some(1),
        query_hash: "abc",
        result_hash: "def",
        endpoint: SparqlEndpoint::Qlever,
        sparql_query: Some("SELECT ?c WHERE {}"),
        year: YEAR,
        date_created: NOW,
    });
    assert!(body.contains("\"@type\": \"Dataset\""));
    assert!(body.contains("\"query_hash\""));
    assert!(!body.contains("LOTUS Knowledge Search"));

    let parsed: Value = serde_json::from_str(&body).expect("valid JSON");
    assert!(parsed.get("variableMeasured").is_some());
    assert_eq!(
        at(&parsed, "/keywords"),
        json!([
            "natural products",
            "chemical structure",
            "taxonomy",
            "SPARQL",
            "linked open data",
            lotus_jsonld::edam::TOPIC_SMALL_MOLECULES,
            lotus_jsonld::edam::TOPIC_CHEMINFORMATICS,
            lotus_jsonld::edam::TOPIC_METABOLOMICS,
            lotus_jsonld::edam::TOPIC_TAXONOMY,
        ])
    );
    let dl_val = at(&parsed, "/distribution/0");
    let dl = dl_val.as_object().expect("DataDownload object");
    assert_eq!(
        dl.len(),
        2,
        "DataDownload carries only @type and encodingFormat when offline: {dl:?}"
    );
    assert_eq!(
        at(&parsed, "/provider/1/url"),
        json!("https://www.wikidata.org/")
    );
    assert_eq!(at(&parsed, "/sparql_query"), json!("SELECT ?c WHERE {}"));
    let issues = lotus_jsonld::check(&parsed, lotus_jsonld::Profile::Dataset);
    assert!(
        issues.is_empty(),
        "exported metadata.json must conform to Bioschemas Dataset 1.0-RELEASE with zero violations and zero warnings: {issues:?}"
    );
    assert_eq!(
        at(&parsed, "/distribution/0/encodingFormat"),
        json!(["text/csv", lotus_jsonld::edam::FORMAT_CSV])
    );
    let taxon_node = at(&parsed, "/about/4");
    let taxon_issues = lotus_jsonld::check(&taxon_node, lotus_jsonld::Profile::Taxon);
    assert!(
        !taxon_issues.iter().any(|i| i.severity == "Violation"),
        "embedded Taxon node in Dataset.about must conform to Bioschemas Taxon 1.0-RELEASE: {taxon_issues:?}"
    );
    assert_eq!(
        at(&taxon_node, "/@id"),
        json!("http://www.wikidata.org/entity/Q42")
    );
}

#[test]
fn metadata_json_shows_wdqs_when_fallback_used() {
    let criteria = SearchCriteria::up_to_year(YEAR);
    let body = build_metadata_json(MetadataInputs {
        criteria: &criteria,
        qid: Some("Q42"),
        number_of_records_override: Some(1),
        query_hash: "abc",
        result_hash: "def",
        endpoint: SparqlEndpoint::Wdqs,
        sparql_query: None,
        year: YEAR,
        date_created: NOW,
    });
    assert!(body.contains(&format!("\"url\": \"{}\"", lotus_model::WDQS_WIKIDATA)));
    assert!(body.contains("Wikidata Query Service"));
    assert!(body.contains("QLever fallback"));
}

#[test]
fn metadata_json_carries_the_creation_stamp_it_was_given() {
    let criteria = SearchCriteria::up_to_year(YEAR);
    let body = build_metadata_json(MetadataInputs {
        criteria: &criteria,
        qid: Some("Q42"),
        number_of_records_override: Some(1),
        query_hash: "abc",
        result_hash: "def",
        endpoint: SparqlEndpoint::Qlever,
        sparql_query: None,
        year: YEAR,
        date_created: NOW,
    });
    assert!(
        body.contains(&format!("\"dateCreated\": \"{NOW}\"")),
        "{body}"
    );
}

/// The document built for `criteria` and `qid`, parsed back.
fn document(criteria: &SearchCriteria, qid: Option<&str>) -> Value {
    let body = build_metadata_json(MetadataInputs {
        criteria,
        qid,
        number_of_records_override: None,
        query_hash: "abc",
        result_hash: "def",
        endpoint: SparqlEndpoint::Qlever,
        sparql_query: None,
        year: YEAR,
        date_created: NOW,
    });
    serde_json::from_str(&body).expect("the metadata document is JSON")
}

/// The value at a JSON pointer, `Null` when nothing is there.
fn at(doc: &Value, pointer: &str) -> Value {
    doc.pointer(pointer).cloned().unwrap_or(Value::Null)
}

fn taxon_and_qid(taxon: &str, qid: Option<&str>) -> (Value, Value) {
    let criteria = SearchCriteria {
        taxon: taxon.into(),
        ..SearchCriteria::up_to_year(YEAR)
    };
    let doc = document(&criteria, qid);
    (
        at(&doc, "/search_parameters/taxon"),
        at(&doc, "/search_parameters/taxon_qid"),
    )
}

#[test]
fn the_effective_taxon_prefers_the_typed_name_then_the_qid() {
    assert_eq!(
        taxon_and_qid("Gentiana", Some("*")),
        (json!("all taxa"), Value::Null)
    );
    assert_eq!(taxon_and_qid("  ", None), (json!("all taxa"), Value::Null));
    assert_eq!(
        taxon_and_qid(" Gentiana ", Some("Q42")),
        (json!("Gentiana"), json!("Q42"))
    );
    assert_eq!(taxon_and_qid("", Some("Q42")), (json!("Q42"), json!("Q42")));
    assert_eq!(
        taxon_and_qid("Gentiana", None),
        (json!("Gentiana"), Value::Null)
    );
}

#[test]
fn a_search_with_no_filters_carries_no_filter_block() {
    let criteria = SearchCriteria {
        taxon: "Gentiana".into(),
        ..SearchCriteria::up_to_year(YEAR)
    };
    let doc = document(&criteria, Some("Q42"));
    assert_eq!(doc.pointer("/search_parameters/filters"), None);
    assert_eq!(at(&doc, "/name"), json!("LOTUS Data — Gentiana"));
}

#[test]
fn a_structure_search_names_its_mode_and_reports_its_filters() {
    let criteria = SearchCriteria {
        taxon: "*".into(),
        structure: "CCO".into(),
        structure_search: lotus_model::SmilesSearchType::Similarity,
        structure_threshold: 0.8,
        ..SearchCriteria::up_to_year(YEAR)
    };
    let doc = document(&criteria, Some("*"));

    assert_eq!(
        at(&doc, "/name"),
        json!("LOTUS Data — Similarity search in all taxa")
    );
    assert_eq!(
        at(&doc, "/about").as_array().map(Vec::len),
        Some(4),
        "wildcard taxon must not append a Taxon node beyond the 4 EDAM DefinedTerms"
    );
    assert_eq!(
        at(&doc, "/search_parameters/structure_query"),
        json!({
            "param_key": "structure",
            "legacy_param_key": "smiles",
            "search_type": "similarity",
            "input_format": "smiles",
            "similarity_threshold": 0.8,
            "query_text": "CCO",
        })
    );
    assert_eq!(
        at(&doc, "/chemical_search_service/endpoint"),
        json!(lotus_model::IDSM_SACHEM_ENDPOINT)
    );
    assert_eq!(
        at(&doc, "/search_parameters/filters"),
        json!({
            "chemical_structure": {
                "smiles": "CCO",
                "search_type": "similarity",
                "similarity_threshold": 0.8,
            }
        })
    );
}

#[test]
fn exact_smiles_search_credits_sachem_endpoint() {
    let criteria = SearchCriteria {
        taxon: "*".into(),
        structure: "CCO".into(),
        structure_search: lotus_model::SmilesSearchType::Exact,
        ..SearchCriteria::up_to_year(YEAR)
    };
    let doc = document(&criteria, Some("*"));
    assert_eq!(at(&doc, "/chemical_search_service/name"), json!("SACHEM"));
    assert_eq!(
        at(&doc, "/chemical_search_service/endpoint"),
        json!(lotus_model::IDSM_SACHEM_ENDPOINT)
    );
}

#[test]
fn a_structure_with_a_line_break_is_reported_as_a_molfile() {
    let structure = "mol\nM  END";
    let criteria = SearchCriteria {
        taxon: "*".into(),
        structure: structure.into(),
        structure_search: lotus_model::SmilesSearchType::Exact,
        ..SearchCriteria::up_to_year(YEAR)
    };
    let doc = document(&criteria, None);

    assert_eq!(
        at(&doc, "/search_parameters/structure_query"),
        json!({
            "param_key": "structure",
            "legacy_param_key": "smiles",
            "search_type": "exact",
            "input_format": "molfile",
            "query_preview": structure,
            "query_length": structure.len(),
        })
    );
}

#[test]
fn endpoints_display_as_their_names() {
    assert_eq!(SparqlEndpoint::Qlever.to_string(), "QLever");
    assert_eq!(SparqlEndpoint::Wdqs.to_string(), "Wikidata Query Service");
}

#[test]
fn exact_qid_lookup_omits_sachem_and_records_reference_and_nomenclature() {
    let mut criteria = SearchCriteria {
        taxon: "Gentiana".into(),
        reference: "10.7554/eLife.70780".into(),
        structure: "Q23118".into(),
        structure_search: lotus_model::SmilesSearchType::Exact,
        ..SearchCriteria::up_to_year(YEAR)
    };
    criteria.taxon_names.basionyms = false;
    let doc = document(&criteria, Some("Q158572"));

    assert_eq!(doc.pointer("/chemical_search_service"), None);
    assert!(
        !at(&doc, "/description")
            .as_str()
            .unwrap_or_default()
            .contains("SACHEM"),
        "exact Wikidata compound QID lookup must not credit SACHEM: {doc}"
    );
    assert_eq!(
        at(&doc, "/search_parameters/reference"),
        json!("10.7554/eLife.70780")
    );
    assert_eq!(
        at(&doc, "/search_parameters/taxon_nomenclature/basionyms"),
        json!(false)
    );
}
