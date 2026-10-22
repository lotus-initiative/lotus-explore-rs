// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `metadata`, in their own file.

use super::*;

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
        year: YEAR,
        date_created: NOW,
    });
    assert!(body.contains("\"@type\": \"Dataset\""));
    assert!(body.contains("\"query_hash\""));
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
        year: YEAR,
        date_created: NOW,
    });
    assert!(body.contains(&format!("\"url\": \"{WDQS_ENDPOINT}\"")));
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
        year: YEAR,
        date_created: NOW,
    });
    assert!(body.contains(&format!("\"dateCreated\": \"{NOW}\"")), "{body}");
}
