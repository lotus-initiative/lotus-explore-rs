// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `wikidata`, in their own file.

#![allow(clippy::expect_used)]

use super::*;

/// `MIN(?compound) AS ?compound` is a QLever `400` ("The target ?compound of
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
    // The query as the code builds it, with one key, so the assertion below is
    // about the real string and not about a copy of it.
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
        "Q22918685".to_string(),
        "Q202864".to_string(),
        "Q22330782".to_string(),
    )];

    assert_as_targets_are_not_used_elsewhere(&build_occurrence_query(&pairs));
    assert_as_targets_are_not_used_elsewhere(&build_occurrence_with_ref_query(&triples));
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
    // Three keys, six quote characters, and therefore three literals. A `VALUES`
    // that quietly lost an entry asks about a compound this run was never
    // given, which reads as "not in Wikidata" for something nobody asked about.
    assert_eq!(
        query.matches('"').count(),
        keys.len() * 2,
        "one literal per key, and no more:\n{query}"
    );
}

#[test]
fn a_batched_occurrence_query_names_its_pairs_as_items_not_literals() {
    // `VALUES` with a quoted string there matches a literal, and every row of
    // this pattern is an IRI. A query that quietly matches nothing is the
    // failure mode: the answer would be "not recorded" for every row, and a
    // curator would submit statements that are already there.
    let query = build_occurrence_query(&[("Q153".to_string(), "Q15978631".to_string())]);
    assert!(query.contains("(wd:Q153 wd:Q15978631)"), "{query}");
    assert!(
        !query.contains("\"Q153\""),
        "a QID is not a literal:\n{query}"
    );
}

/// A name the batch cannot resolve is one the row loop re-asks, one request at
/// a time. The batch tried only the raw spelling while the single-row lookup
/// also tried the canonicalised one, so every row needing canonicalisation
/// paid for a second identical request. Both spellings now go into the one
/// batched query under one lookup key.
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
fn normalize_taxon_lookup_trims_and_lowercases() {
    assert_eq!(
        normalize_taxon_lookup("  Gentiana lutea  "),
        Some("gentiana lutea".to_string())
    );
    assert_eq!(normalize_taxon_lookup("   \n"), None);
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
fn build_single_taxon_lookup_query_uses_values_without_lcase_filter() {
    let query = build_single_taxon_lookup_query("ficticia imaginaria").expect("query");

    assert!(query.contains("VALUES ?taxonName"));
    assert!(query.contains("\"ficticia imaginaria\" \"Ficticia imaginaria\""));
    assert!(query.contains("wdt:P31 wd:Q16521"));
    assert!(!query.contains("LCASE"));
    assert!(!query.contains("FILTER"));
}

#[test]
fn build_reference_lookup_query_uses_values_pairs() {
    let query = build_reference_lookup_query(&["10.1000/ABC".into(), "10.2000/XYZ".into()]);

    assert!(query.contains("VALUES (?lookup ?doi)"));
    assert!(query.contains("\"10.1000/ABC\" \"10.1000/ABC\""));
    assert!(query.contains("?ref wdt:P356 ?doi"));
}
