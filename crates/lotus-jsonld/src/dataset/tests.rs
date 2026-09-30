// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for the dataset documents, in their own file.

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
