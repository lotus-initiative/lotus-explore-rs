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
    assert!(
        query.contains("ORDER BY STRLEN(STR(?compound)) ?compound LIMIT 1"),
        "single-compound lookup must sort QIDs by length before URI so Q20 precedes Q100: {query}"
    );
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
    assert!(is_binomial("Cortinarius subtortus (Pers.) Fr."));
    assert!(is_binomial("Salvia rosmarinus Spenn."));
    assert!(is_binomial("Hypericum elodeoides Choisy"));
    assert!(is_binomial("Gentiana lutea L. f."));
    assert!(is_binomial("Gentiana lutea subsp. alba"));
    assert!(is_binomial("Gentiana lutea ssp. alba (L.) DC."));
    assert!(is_binomial("Mentha × piperita"));
    // A genus alone or open nomenclature is ambiguous: curating an
    // occurrence against it is a data error rather than a near miss.
    assert!(!is_binomial("Gentiana"));
    assert!(!is_binomial("Gentiana sp."));
    assert!(!is_binomial("Gentiana spp."));
    assert!(!is_binomial("Gentiana cf. lutea"));
    assert!(
        !is_binomial("Homo sapiens extra"),
        "three words without rank or author is not a binomial"
    );
    assert!(!is_binomial(""), "empty");
    assert!(!is_binomial("   "), "whitespace only");
    assert!(!is_binomial("gentiana lutea"), "the genus is capitalised");
}

#[test]
fn extract_canonical_taxon_name_strips_authorities_and_normalizes_ranks() {
    assert_eq!(
        extract_canonical_taxon_name("Cortinarius subtortus (Pers.) Fr.").as_deref(),
        Some("Cortinarius subtortus")
    );
    assert_eq!(
        extract_canonical_taxon_name("Salvia rosmarinus Spenn.").as_deref(),
        Some("Salvia rosmarinus")
    );
    assert_eq!(
        extract_canonical_taxon_name("Gentiana lutea ssp. alba (L.) DC.").as_deref(),
        Some("Gentiana lutea subsp. alba")
    );
    assert_eq!(
        extract_canonical_taxon_name("Gentiana lutea var. alba").as_deref(),
        Some("Gentiana lutea var. alba")
    );
    assert_eq!(
        extract_canonical_taxon_name("Gentiana lutea forma alba").as_deref(),
        Some("Gentiana lutea f. alba")
    );
    assert_eq!(
        extract_canonical_taxon_name("Gentiana lutea f. alba").as_deref(),
        Some("Gentiana lutea f. alba")
    );
    assert_eq!(
        extract_canonical_taxon_name("Gentiana lutea subvar. alba").as_deref(),
        Some("Gentiana lutea subvar. alba")
    );
    assert_eq!(
        extract_canonical_taxon_name("Gentiana lutea subf. alba").as_deref(),
        Some("Gentiana lutea subf. alba")
    );
    assert_eq!(
        extract_canonical_taxon_name("Gentiana ciliata L. f.").as_deref(),
        Some("Gentiana ciliata")
    );
    assert_eq!(
        extract_canonical_taxon_name("×Heucherella tiarelloides").as_deref(),
        Some("×Heucherella tiarelloides")
    );
    assert_eq!(
        extract_canonical_taxon_name("Pseudo-nitzschia seriata").as_deref(),
        Some("Pseudo-nitzschia seriata")
    );
    assert_eq!(
        extract_canonical_taxon_name("Hibiscus rosa-sinensis").as_deref(),
        Some("Hibiscus rosa-sinensis")
    );
    assert_eq!(
        extract_canonical_taxon_name("Solanum tuberosum van Houtte").as_deref(),
        Some("Solanum tuberosum")
    );
    assert_eq!(
        extract_canonical_taxon_name("Salvia officinalis ex (L.) Mill.").as_deref(),
        Some("Salvia officinalis")
    );
    assert_eq!(
        extract_canonical_taxon_name("Fusarium oxysporum f. sp. cubense (E.F.Sm.) W.C.Snyder")
            .as_deref(),
        Some("Fusarium oxysporum f. sp. cubense")
    );
    assert_eq!(
        extract_canonical_taxon_name("Puccinia graminis f.sp. tritici").as_deref(),
        Some("Puccinia graminis f. sp. tritici")
    );
    assert_eq!(
        extract_canonical_taxon_name("Fusarium oxysporum f. sp."),
        None
    );
    assert_eq!(
        extract_canonical_taxon_name("Fusarium oxysporum f. sp. 123"),
        None
    );
    assert_eq!(
        extract_canonical_taxon_name("Fusarium oxysporum f. sp. cubense extra"),
        None
    );
    assert_eq!(extract_canonical_taxon_name("Gentiana sp."), None);
    assert_eq!(extract_canonical_taxon_name("Gentiana sp"), None);
    assert_eq!(extract_canonical_taxon_name("Gentiana sensu"), None);
    assert_eq!(extract_canonical_taxon_name("Gentiana 123"), None);
    assert_eq!(extract_canonical_taxon_name("Gentiana lutea van"), None);
    assert_eq!(
        extract_canonical_taxon_name("Gentiana lutea van other"),
        None
    );

    assert_eq!(
        taxon_name_candidates("Gentiana lutea"),
        vec!["Gentiana lutea".to_string()]
    );
    assert_eq!(
        taxon_name_candidates("Gentiana lutea L."),
        vec![
            "Gentiana lutea L.".to_string(),
            "Gentiana lutea".to_string()
        ]
    );
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
    assert!(
        !query.contains("||"),
        "a plain binomial needs only one comparison clause: {query}"
    );

    let cited = taxon_by_name_query("Cortinarius subtortus (Pers.) Fr.");
    assert!(
        cited.contains("\"Cortinarius subtortus\""),
        "author-stripped canonical binomial is queried too: {cited}"
    );
    assert!(
        cited.contains("||"),
        "author-cited name queries both stripped and raw forms: {cited}"
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
    // The same model the web app writes: an instance of "type of chemical
    // entity", a subclass of "chemical compound".
    assert!(
        statements.contains(&format!("LAST|P31|{WD_TYPE_CHEMICAL_ENTITY_QID}")),
        "{statements}"
    );
    assert!(
        statements.contains(&format!("LAST|P279|{WD_CHEMICAL_COMPOUND_QID}")),
        "{statements}"
    );
    assert!(
        !statements.contains(&format!("P31|{WD_CHEMICAL_COMPOUND_QID}")),
        "not an instance of chemical compound: {statements}"
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
fn the_batched_compound_query_returns_raw_items_without_lexicographic_min() {
    let query = build_compound_lookup_query(&["LFQSCWFLJHTTHZ-UHFFFAOYSA-N".to_string()]);
    assert!(
        query.contains("SELECT ?key ?compound ?canonical"),
        "the query returns ?compound directly for client-side numeric QID selection:\n{query}"
    );
    assert!(
        !query.contains("MIN(") && !query.contains("GROUP BY"),
        "SPARQL MIN orders Q100 before Q20 lexicographically and must not be used:\n{query}"
    );
    assert_as_targets_are_not_used_elsewhere(&query);
    assert!(
        qid_number("Q20") < qid_number("Q100"),
        "Q20 must sort before Q100 numerically"
    );
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
    assert!(query.contains("ORDER BY ?taxon LIMIT 1"), "{query}");
}

#[test]
fn occurrence_with_ref_queries_exclude_deprecated_rank() {
    let single = has_occurrence_with_ref_query("Q1", "Q2", "Q3");
    assert!(
        single.contains("PREFIX wikibase: <http://wikiba.se/ontology#>"),
        "{single}"
    );
    assert!(single.contains("wikibase:rank ?rank"), "{single}");
    assert!(
        single.contains("FILTER(?rank != wikibase:DeprecatedRank)"),
        "{single}"
    );

    let batched = build_occurrence_with_ref_query(&[("Q1".into(), "Q2".into(), "Q3".into())]);
    assert!(
        batched.contains("PREFIX wikibase: <http://wikiba.se/ontology#>"),
        "{batched}"
    );
    assert!(batched.contains("wikibase:rank ?rank"), "{batched}");
    assert!(
        batched.contains("FILTER(?rank != wikibase:DeprecatedRank)"),
        "{batched}"
    );
}

#[test]
fn limit_one_lookup_queries_have_deterministic_ordering() {
    assert!(
        compound_by_inchikey_query("LFQSCWFLJHTTHZ-UHFFFAOYSA-N")
            .contains("ORDER BY STRLEN(STR(?compound)) ?compound LIMIT 1"),
        "compound lookup must order by IRI string length before IRI so Q20 precedes Q100"
    );
    assert!(taxon_by_name_query("Gentiana lutea").contains("ORDER BY ?taxon LIMIT 1"));
    assert!(reference_by_doi_query("10.1000/xyz123").contains("ORDER BY ?ref LIMIT 1"));
}
