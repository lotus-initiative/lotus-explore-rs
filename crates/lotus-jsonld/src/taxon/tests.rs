// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `taxon`, in their own file.

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
