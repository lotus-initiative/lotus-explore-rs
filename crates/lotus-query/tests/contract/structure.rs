// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//!
//! Structure search, as SPARQL.
//!
//! A structure search without a taxon keeps its occurrences optional, and with a
//! taxon requires them. Similarity and substructure ask the service for different
//! things, and the query has to say which.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "a test that fails on a bad fixture is reporting, not panicking"
)]
use lotus_model::SmilesSearchType;
use lotus_query::{
    Nomenclature, escape_structure_literal, structure_search_query, structure_search_query_with,
};

#[test]
fn a_structure_search_without_a_taxon_keeps_optional_occurrences() {
    // Structure search is how a compound with no occurrence data is found in
    // the first place, so requiring a taxon would hide exactly those.
    let query = structure_search_query("CCO", SmilesSearchType::Substructure, 0.8, None);
    assert!(query.contains("SERVICE idsm:wikidata"));
    assert!(query.contains("sachem:substructureSearch"));
    assert!(query.contains("OPTIONAL {"));
    assert!(query.contains("?c p:P703 ?statement ."));
    assert!(!query.contains("P171*"), "no taxon to filter on");
    // The service must be in the body, or nothing is searched at all.
    assert!(query.contains("WHERE {"), "the query has a WHERE block");
}

#[test]
fn a_structure_search_with_a_taxon_requires_the_occurrence() {
    let query = structure_search_query_with(
        "CCO",
        SmilesSearchType::Substructure,
        0.8,
        Some("Q16521"),
        &Nomenclature::ALL_OFF,
    );
    assert!(
        query.contains("SELECT DISTINCT ?c"),
        "the service is pre-filtered"
    );
    assert!(query.contains("?t (wdt:P171*) wd:Q16521 ."));
    assert!(
        !query.contains("OPTIONAL {\n    ?c p:P703"),
        "a match outside the taxon must not come back"
    );
}

#[test]
fn similarity_and_substructure_ask_the_service_for_different_things() {
    let sim = structure_search_query("CCO", SmilesSearchType::Similarity, 0.75, None);
    assert!(sim.contains("sachem:similarCompoundSearch"));
    assert!(sim.contains(r#"sachem:cutoff "0.75"^^xsd:double"#));
    assert!(!sim.contains("scoredSubstructureSearch"));

    let sub = structure_search_query("CCO", SmilesSearchType::Substructure, 0.75, None);
    assert!(sub.contains("sachem:substructureSearch"));
    assert!(
        !sub.contains("sachem:cutoff"),
        "a cutoff means nothing to substructure search"
    );
}

#[test]
fn a_multiline_structure_uses_the_scored_service() {
    const MOLFILE: &str = "\n\n\n  1  0  0  0  0  0  0  0  0  0999 V2000\n    0.0000    0.0000    0.0000 C   0  0  0  0  0  0\nM  END\n";
    let query = structure_search_query(MOLFILE, SmilesSearchType::Substructure, 0.8, None);
    assert!(query.contains("sachem:scoredSubstructureSearch"));
    assert!(query.contains("sachem:searchMode sachem:substructureSearch"));
    assert!(query.contains("sachem:topn \"-1\"^^xsd:integer"));
    assert!(
        query.contains("'''"),
        "a multi-line literal needs triple quotes"
    );
}

#[test]
fn a_structure_is_escaped_before_it_reaches_the_query() {
    let query = structure_search_query(r#"C"C"#, SmilesSearchType::Substructure, 0.8, None);
    assert!(query.contains(r#"sachem:query "C\"C""#));
}

#[test]
fn a_multi_line_smiles_is_triple_quoted() {
    // The other case where the right side holds and the left does not: SMILES
    // written across lines, which is how a reaction or a pasted structure
    // arrives. A newline inside a double-quoted literal is not a newline.
    assert_eq!(
        escape_structure_literal("CCO\n.O"),
        "'''CCO
.O'''"
    );
}

#[test]
fn a_single_line_smiles_is_double_quoted_with_escapes() {
    assert_eq!(escape_structure_literal("  CCO  "), r#""CCO""#);
    assert_eq!(escape_structure_literal(r#"a"b"#), r#""a\"b""#);
    assert_eq!(escape_structure_literal(r"a\b"), r#""a\\b""#);
}

#[test]
fn a_single_line_smiles_substructure_search_uses_the_cheap_service() {
    // The `Substructure if is_multiline` arm exists because a molfile has to go
    // through the scored search, which understands atom mapping, while a SMILES
    // does not. With the guard forced true, every SMILES substructure search
    // takes the expensive path and asks for scores the caller never reads.
    let smiles = structure_search_query("CCO", SmilesSearchType::Substructure, 0.8, None);
    assert!(
        !smiles.contains("scoredSubstructureSearch"),
        "a single-line SMILES should use the plain substructure service"
    );
    assert!(smiles.contains("sachem:substructureSearch"));

    // The same search with a molfile does need the scored path, which is the
    // half of the guard that distinguishes it from the arm above.
    let molfile = structure_search_query(
        "ethanol\n  0  0  0  0  0  0  0  0  0  0999 V2000\nM  END",
        SmilesSearchType::Substructure,
        0.8,
        None,
    );
    assert!(
        molfile.contains("scoredSubstructureSearch"),
        "a molfile substructure search needs the scored path"
    );
}
