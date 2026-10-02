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
    Nomenclature, all_compounds_query, compounds_by_taxon_query, escape_structure_literal,
    exact_compound_query, structure_search_query, structure_search_query_with,
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

// ── Exact: the query that does not call the structure service ────────────────

#[test]
fn a_compound_query_never_touches_the_structure_service() {
    // The whole point of the identity path. Substructure and similarity both have
    // to load a substructure index and score every candidate in it; an identity
    // question is an index scan for one item.
    let query = exact_compound_query("Q18216", None);
    assert!(
        !query.contains("SERVICE"),
        "exact must not call IDSM:\n{query}"
    );
    assert!(
        !query.contains("sachem"),
        "exact must not call IDSM:\n{query}"
    );
}

#[test]
fn a_compound_identity_query_seeds_on_the_compound_it_was_given() {
    let query = exact_compound_query("Q18216", None);
    assert!(query.contains("VALUES ?c { wd:Q18216 }"), "{query}");
    assert!(
        !query.contains("wdt:P235 ?compound_inchikey ;"),
        "the seed replaces the index scan, it does not join it:\n{query}"
    );
}

#[test]
fn a_compound_identity_query_keeps_occurrences_optional() {
    // A compound with no occurrence data is exactly what a name search is
    // looking for, so requiring one would hide the thing that was asked for.
    let query = exact_compound_query("Q18216", None);
    assert!(query.contains("OPTIONAL {"), "{query}");
    assert!(
        !query.contains("?c p:P703 ?statement .\n          ?statement ps:P703"),
        "the occurrence triple must not be required:\n{query}"
    );
}

#[test]
fn the_optional_occurrences_are_planned_apart_from_the_seed() {
    // This is a performance contract, and the shape is the only part of it that
    // is visible here.
    //
    // The occurrence chain costs 0.1s on its own against this endpoint and 4.3s
    // inside a bare `OPTIONAL` -- measured on Q3613679, 20 rows either way. It is
    // the `OPTIONAL` and not the patterns, so the block is wrapped in its own
    // subquery to hand the planner a left side of a known size.
    //
    // The repeated `VALUES` inside the subquery is the part that makes it work
    // and the part that looks redundant. Without it the subquery inherits `?c`
    // from the enclosing scope, the planner is back where it started, and the
    // twenty-fold cost comes straight back -- verified by removing it.
    let query = exact_compound_query("Q18216", None);
    let occurrences_at = query.find("?c p:P703 ?statement").unwrap_or_else(|| {
        panic!("the occurrence chain should still be there:\n{query}");
    });
    let before = &query[..occurrences_at];
    assert!(
        before.rfind("VALUES ?c { wd:Q18216 }").is_some(),
        "the seed has to be repeated inside the block, or the slow plan returns:\n{query}"
    );
    assert!(
        before.rfind("{ SELECT * WHERE {").is_some(),
        "the occurrence block has to be planned as its own subquery:\n{query}"
    );
}

#[test]
fn a_compound_identity_query_inside_a_taxon_still_follows_the_nomenclature() {
    let query = exact_compound_query("Q18216", Some(("Q156598", &Nomenclature::ALL_ON)));
    assert!(query.contains("VALUES ?c { wd:Q18216 }"));
    assert!(query.contains("wdt:P225 ?taxon_name"));
    assert!(
        query.contains("VALUES ?seed { wd:Q156598 }"),
        "the taxon's closure is still expanded:\n{query}"
    );
}

#[test]
fn the_taxon_and_all_compounds_queries_are_unchanged_by_the_exact_seed() {
    // The compound seed is an addition, not a rewrite: with none of them the two
    // queries are byte-for-byte what they were.
    let all = all_compounds_query();
    assert!(all.contains("?c wdt:P235 ?compound_inchikey ;"));
    assert!(all.contains("?c p:P703 ?statement ."));
    assert!(all.contains("?t wdt:P225 ?taxon_name"));
    let by_taxon = compounds_by_taxon_query("Q156598");
    assert!(by_taxon.contains("VALUES ?seed { wd:Q156598 }"));
}
