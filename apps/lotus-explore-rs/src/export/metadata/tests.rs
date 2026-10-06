// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `metadata`, in their own file.

use super::*;

#[test]
fn metadata_json_contains_schema_dataset() {
    let criteria = SearchCriteria::up_to_year(crate::clock::current_year());
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
    let criteria = SearchCriteria::up_to_year(crate::clock::current_year());
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
