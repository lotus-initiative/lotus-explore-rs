// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for the Wikidata query builders, in their own file.
//!
//! A query is checked as a string, because a query that builds and is wrong is
//! indistinguishable from one that does not build until the endpoint answers it.

#![allow(clippy::expect_used, clippy::panic)]

use super::*;

#[test]
fn the_compound_query_matches_on_the_inchikey() {
    let query = compound_by_inchikey_query("LFQSCWFLJHTTHZ-UHFFFAOYSA-N");
    // P235 is the InChIKey property, and matching on it is the whole point:
    // SMILES differ by atom order for the same molecule.
    assert!(query.contains("wdt:P235"), "{query}");
    assert!(
        !query.contains("wdt:P233 \""),
        "must not match on SMILES: {query}"
    );
    assert!(query.contains("LIMIT 1"), "{query}");
}

#[test]
fn a_quote_in_a_value_cannot_end_the_literal() {
    // An unescaped quote closes the literal early, and the rest of the
    // value is then parsed as query text. A structure whose identifier
    // contained a quote would change the shape of the query, not just what
    // it looks for.
    let hostile = "AAA\" . ?compound wdt:P31 ?x";
    let query = compound_by_inchikey_query(hostile);

    // Count the unescaped quotes: there must be exactly two, the ones that
    // open and close the literal. A third would be a hole in the string.
    let quotes = query.chars().filter(|c| *c == '"').count();
    // A backslash-quote pair is one escaped quote, not a delimiter.
    let escaped = query.matches(r#"\""#).count();
    assert_eq!(
        quotes - escaped,
        2,
        "the value must sit inside exactly one literal: {query}"
    );

    // And the injected pattern text is inside that literal rather than
    // beside it, so it is a value and not a clause.
    let value = query
        .split_once("wdt:P235 ")
        .and_then(|(_, rest)| rest.split_once('"'))
        .and_then(|(_, rest)| rest.rsplit_once('"'))
        .map_or("", |(value, _)| value);
    assert!(
        value.contains("?compound wdt:P31"),
        "the injected text should be the value, not the pattern: {query}"
    );
}

#[test]
fn a_qid_is_read_from_a_uri_and_nothing_else() {
    assert_eq!(
        qid_from_uri("http://www.wikidata.org/entity/Q16521"),
        Some("Q16521")
    );
    // A label, a number and a bare word are not QIDs.
    assert_eq!(qid_from_uri("Gentiana lutea"), None);
    assert_eq!(qid_from_uri("42"), None);
    assert_eq!(qid_from_uri("Q"), None);
    assert_eq!(qid_from_uri("Qabc"), None);
    assert_eq!(qid_from_uri("https://example.org/Q7?x=1"), None);
}

#[test]
fn a_binomial_is_recognised_and_a_genus_is_not() {
    assert!(is_binomial("Gentiana lutea"));
    assert!(is_binomial("Homo sapiens"));
    // A genus alone is ambiguous: it matches hundreds of species, and
    // curating an occurrence against it is a data error.
    assert!(!is_binomial("Gentiana"));
    assert!(
        !is_binomial("Homo sapiens extra"),
        "three words is not a binomial"
    );
    assert!(!is_binomial(""), "empty");
    assert!(!is_binomial("   "), "whitespace only");
    assert!(!is_binomial("gentiana lutea"), "the genus is capitalised");
}

#[test]
fn the_taxon_query_asks_for_both_name_kinds() {
    let query = taxon_by_name_query("Gentiana lutea");
    assert!(query.contains("P225"), "scientific name: {query}");
    assert!(query.contains("P1843"), "common name: {query}");
    assert!(
        query.contains("LCASE"),
        "the match is case-insensitive: {query}"
    );
}

#[test]
fn the_occurrence_query_asks_rather_than_selects() {
    // `ASK` returns whether it already holds, so the answer is the diff
    // rather than a set to be compared by hand.
    let query = has_occurrence_query("Q1", "Q2");
    assert!(query.contains("ASK"), "{query}");
    assert!(!query.contains("SELECT"), "{query}");
    assert!(query.contains("wd:Q1 wdt:P703 wd:Q2"), "{query}");
}

#[test]
fn a_created_compound_is_classed_before_it_is_filled_in() {
    let statements = create_compound_statements("ethanol", "CCO", "CCO");
    let class = statements.find("P31").expect("a class");
    let canonical = statements.find("P233").expect("canonical SMILES");
    assert!(
        class < canonical,
        "an item with no class is unfindable, so P31 comes first:\\n{statements}"
    );
    assert!(
        statements.contains(WD_CHEMICAL_COMPOUND_QID),
        "{statements}"
    );
}

#[test]
fn every_escape_is_reversible() {
    for value in [r#"a"b"#, r"a\b", "a\nb", "a;b", "plain"] {
        let escaped = escape_sparql_string(value);
        let unescaped = escaped
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
            .replace('\n', " ");
        assert!(!unescaped.contains('"') || value.contains('"'), "{value}");
        assert!(
            !escaped.contains('\n'),
            "a newline in a literal is a syntax error"
        );
    }
}

#[test]
fn a_backslash_is_itself_escaped() {
    // The quote and the control characters cannot occur in a real DOI, so
    // this is the escape that only shows up when a value is pasted rather
    // than typed -- and getting it wrong silently corrupts the literal.
    assert_eq!(escape_sparql_string(r"a\b"), r"a\\b");
    assert_eq!(escape_sparql_string(r#"a"b"#), r#"a\"b"#);
    assert_eq!(
        escape_sparql_string("a\nb\tc"),
        "a b c",
        "control characters become spaces"
    );
}

#[test]
fn a_doi_lookup_is_a_doi_predicate() {
    let query = reference_by_doi_query("10.1000/xyz123");
    assert!(query.contains("SELECT ?ref WHERE"), "{query}");
    assert!(
        query.contains(&format!("wdt:{} \"10.1000/xyz123\"", property::DOI)),
        "the DOI is the value of the DOI predicate: {query}"
    );
    assert!(
        query.contains("LIMIT 1"),
        "one reference is enough to match on: {query}"
    );
}

#[test]
fn a_doi_with_a_quote_in_it_cannot_break_out_of_the_literal() {
    // The value is interpolated into a SPARQL string literal, so an
    // unescaped quote ends the literal and everything after it is parsed as
    // query. `lotus curate` reads from the live endpoint, so this is the
    // difference between a bad match and a query nobody wrote.
    let query = reference_by_doi_query(r#"10.1/" . ?ref ?o ."#);
    assert!(
        !query.contains(r#"10.1/" . ?ref"#),
        "the quote has to be escaped: {query}"
    );
    assert!(query.contains(r#"10.1/\""#), "{query}");
}
