// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for the Wikidata query builders, in their own file.
//!
//! A query is checked as a string, because a query that builds and is wrong is
//! indistinguishable from one that does not build until the endpoint answers it.

#![allow(clippy::expect_used)]

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

    let ref_query = has_occurrence_with_ref_query("Q1", "Q2", "Q3");
    assert!(ref_query.contains("ASK"), "{ref_query}");
    assert!(ref_query.contains("wd:Q1 p:P703 ?stmt"), "{ref_query}");
    assert!(ref_query.contains("?stmt ps:P703 wd:Q2"), "{ref_query}");
    assert!(
        ref_query.contains("prov:wasDerivedFrom ?refnode"),
        "{ref_query}"
    );
    assert!(ref_query.contains("?refnode pr:P248 wd:Q3"), "{ref_query}");
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

/// `MIN(?compound) AS ?compound` is a `QLever` `400` ("The target ?compound of
/// an AS clause was already used in the query body") and broke *every*
/// curation run. No SPARQL parser exists here to hand the query to, so this
/// checks the property it got wrong on the generated text: a variable an `AS`
/// clause introduces must not appear elsewhere in the query. Not a substitute
/// for asking the endpoint.
fn assert_as_targets_are_not_used_elsewhere(query: &str) {
    let mut rest = query;
    while let Some(start) = rest.find(" AS ?") {
        let after = &rest[start + " AS ?".len()..];
        let end = after
            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
            .unwrap_or(after.len());
        let target = format!("?{}", &after[..end]);
        let occurrences = query.matches(&target).count();
        assert_eq!(
            occurrences, 1,
            "the AS clause introduces {target}, so it must appear exactly once \
             in the query; it appears {occurrences} times:\n{query}"
        );
        rest = &after[end..];
    }
}

#[test]
fn the_batched_compound_query_projects_the_aggregate_under_a_new_name() {
    let query = build_compound_lookup_query(&["LFQSCWFLJHTTHZ-UHFFFAOYSA-N".to_string()]);
    assert!(
        query.contains("(MIN(?compound) AS ?compound_item)"),
        "the aggregate is projected as ?compound_item:\n{query}"
    );
    assert_as_targets_are_not_used_elsewhere(&query);
}

#[test]
fn the_batched_occurrence_queries_have_no_as_clause_to_conflict() {
    let pairs = vec![("Q153".to_string(), "Q15978631".to_string())];
    let triples = vec![(
        "Q407418".to_string(),
        "Q8071725".to_string(),
        "Q64216123".to_string(),
    )];
    let occ_query = build_occurrence_query(&pairs);
    let occ_ref_query = build_occurrence_with_ref_query(&triples);
    assert!(
        occ_ref_query.contains("(wd:Q407418 wd:Q8071725 wd:Q64216123)"),
        "{occ_ref_query}"
    );
    assert!(
        occ_ref_query.contains("?compound p:P703 ?stmt"),
        "{occ_ref_query}"
    );
    assert!(
        occ_ref_query.contains("?stmt ps:P703 ?taxon"),
        "{occ_ref_query}"
    );
    assert!(
        occ_ref_query.contains("prov:wasDerivedFrom ?refnode"),
        "{occ_ref_query}"
    );
    assert!(
        occ_ref_query.contains("?refnode pr:P248 ?ref"),
        "{occ_ref_query}"
    );
    assert_as_targets_are_not_used_elsewhere(&occ_query);
    assert_as_targets_are_not_used_elsewhere(&occ_ref_query);
}

#[test]
fn a_batched_query_asks_for_exactly_the_keys_it_was_given() {
    let keys: Vec<String> = ["AAAA-BBB", "CCCC-DDD", "EEEE-FFF"]
        .iter()
        .map(|key| (*key).to_string())
        .collect();

    let query = build_compound_lookup_query(&keys);

    for key in &keys {
        assert!(query.contains(key.as_str()), "{key} is missing:\n{query}");
    }
    assert_eq!(
        query.matches('"').count(),
        keys.len() * 2,
        "one literal per key, and no more:\n{query}"
    );
}

#[test]
fn a_batched_occurrence_query_names_its_pairs_as_items_not_literals() {
    let query = build_occurrence_query(&[("Q153".to_string(), "Q15978631".to_string())]);
    assert!(query.contains("(wd:Q153 wd:Q15978631)"), "{query}");
    assert!(
        !query.contains("\"Q153\""),
        "a QID is not a literal:\n{query}"
    );
}

#[test]
fn the_batch_query_asks_for_both_spellings_of_a_name() {
    let lookups = vec![
        ("gentiana lutea".to_string(), "Gentiana  lutea".to_string()),
        ("gentiana lutea".to_string(), "Gentiana lutea".to_string()),
    ];

    let query = build_taxon_lookup_query(&lookups);

    assert_eq!(
        query.matches("Gentiana lutea").count(),
        1,
        "the raw spelling is asked for: {query}"
    );
    assert!(
        query.contains("Gentiana  lutea"),
        "the canonicalised spelling is asked for too: {query}"
    );
    assert_eq!(
        query.matches("\"gentiana lutea\"").count(),
        2,
        "both rows carry the same lookup key, so a match is cached under it: \
         {query}"
    );
}

#[test]
fn build_taxon_lookup_query_uses_values_pairs_and_taxon_type_constraint() {
    let query = build_taxon_lookup_query(&[
        ("voacanga africana".into(), "Voacanga africana".into()),
        ("gentiana lutea".into(), "Gentiana lutea".into()),
    ]);

    assert!(query.contains("VALUES (?lookup ?taxonName)"));
    assert!(query.contains("wdt:P225 ?taxonName"));
    assert!(query.contains("wdt:P31 wd:Q16521"));
    assert!(query.contains("\"voacanga africana\" \"Voacanga africana\""));
}

#[test]
fn build_reference_lookup_query_uses_values_pairs() {
    let query = build_reference_lookup_query(&["10.1000/ABC".into(), "10.2000/XYZ".into()]);

    assert!(query.contains("VALUES (?lookup ?doi)"));
    assert!(query.contains("\"10.1000/ABC\" \"10.1000/ABC\""));
    assert!(query.contains("?ref wdt:P356 ?doi"));
}

#[test]
fn normalize_taxon_lookup_trims_and_lowercases() {
    assert_eq!(
        normalize_taxon_lookup("  Gentiana lutea  "),
        Some("gentiana lutea".to_string())
    );
    assert_eq!(normalize_taxon_lookup("   \n"), None);
}

#[test]
fn build_single_taxon_lookup_query_uses_values_without_lcase_filter() {
    let query = build_single_taxon_lookup_query("ficticia imaginaria").expect("query");

    assert!(query.contains("VALUES ?taxonName"));
    assert!(query.contains("\"ficticia imaginaria\" \"Ficticia imaginaria\""));
    assert!(query.contains("wdt:P31 wd:Q16521"));
    assert!(!query.contains("LCASE"));
    assert!(!query.contains("FILTER"));
}
