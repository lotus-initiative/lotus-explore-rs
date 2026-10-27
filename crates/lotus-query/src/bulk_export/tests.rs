// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Unit tests for `bulk_export`.

use super::*;

#[test]
fn extract_qid_strips_the_entity_prefix() {
    assert_eq!(extract_qid("http://www.wikidata.org/entity/Q153"), "Q153");
    assert_eq!(extract_qid("Q153"), "Q153");
    assert_eq!(
        extract_qid("http://example.org/not-wikidata"),
        "http://example.org/not-wikidata"
    );
}

#[test]
fn exporter_triplet_query_carries_all_four_variables() {
    let query = query_compound_taxon_reference();
    assert!(query.contains("wdt:P235"));
    assert!(query.contains("p:P703"));
    assert!(query.contains("ps:P703"));
    assert!(query.contains("prov:wasDerivedFrom/pr:P248"));
    assert!(query.contains("?compound_inchikey"));
    assert!(query.contains("?taxon"));
    assert!(query.contains("?reference"));
}

#[test]
fn exporter_reference_queries_all_restrict_to_occurrence_statements() {
    for query in [
        query_reference_doi(),
        query_reference_pmid(),
        query_reference_pmcid(),
        query_reference_title(),
        query_reference_date(),
        query_reference_journal(),
    ] {
        assert!(query.contains("p:P703"), "{query} lacks p:P703");
        assert!(
            query.contains("prov:wasDerivedFrom/pr:P248"),
            "{query} lacks the reference hop"
        );
    }
}

#[test]
fn pubchem_tree_taxon_parent_is_constrained_to_biota() {
    let query = query_taxon_parent();
    assert!(query.contains("wd:Q2382443"));
    assert!(query.contains("wdt:P171*"));
}

#[test]
fn pubchem_compound_parent_is_constrained_to_chemical_compound() {
    let query = query_compound_parent();
    assert!(query.contains("wd:Q11173"));
    assert!(query.contains("wdt:P279*"));
}

#[test]
fn archive_queries_are_paged_by_offset() {
    let page = archive_page(ARCHIVE_SMILES_QUERY, 0);
    assert!(page.contains("LIMIT 50000"));
    assert!(page.contains("OFFSET 0"));
    let page = archive_page(ARCHIVE_SMILES_QUERY, 50_000);
    assert!(page.contains("OFFSET 50000"));
    // The LIMIT is a constant, never derived from the offset argument.
    assert_eq!(page.matches("LIMIT").count(), 1);
}

#[test]
fn archive_inchikey_validation_rejects_non_inchikeys() {
    assert!(looks_like_archive_inchikey("LFQSCWFLJHTTHZ-UHFFFAOYSA-N"));
    assert!(!looks_like_archive_inchikey("not-an-inchikey"));
    assert!(!looks_like_archive_inchikey(""));
}
