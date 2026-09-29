// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The SPARQL each builder must produce.
//!
//! These assert structure rather than bytes: which subquery a fragment lands in,
//! which `OPTIONAL`s the count query drops, which triple the filter injection
//! makes required. Layout may change; the query plan may not.

// The panic lints exist to keep library code free of panics on external input.
// A test that fails on a bad fixture is reporting, not panicking.
#![allow(unused_crate_dependencies)]
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use lotus_core::{ElementState, SearchCriteria, SmilesSearchType};
use lotus_sparql::{
    all_compounds_query, compounds_by_taxon_query, construct_from_select, counts_query,
    export_query, limit_query, structure_search_query, taxon_lookup_query, with_filters,
};

const NOW: u16 = 2026;

const fn criteria() -> SearchCriteria {
    SearchCriteria::up_to_year(NOW)
}

/// `SELECT` opens a subquery at each of these depths.
fn subquery_depth(query: &str) -> usize {
    query.matches("SELECT").count()
}

/// What the base query looks like with its own closing brace removed, which is
/// what the filter fragments are spliced onto.
fn base_body(query: &str) -> &str {
    query
        .trim_end()
        .strip_suffix('}')
        .expect("a built query ends with `}`")
}

#[test]
fn the_compound_query_is_three_levels_deep() {
    for query in [all_compounds_query(), compounds_by_taxon_query("Q16521")] {
        assert_eq!(
            subquery_depth(&query),
            3,
            "SELECT plus two nested subqueries"
        );
        assert!(query.contains("?c wdt:P235 ?compound_inchikey"));
        assert!(query.contains("?c p:P703 ?statement"));
        assert!(query.contains("?statement ps:P703 ?t"));
        assert!(query.contains("prov:wasDerivedFrom ?ref"));
        assert!(query.contains("?ref pr:P248 ?r"));
        assert!(query.contains("?t wdt:P225 ?taxon_name"));
        // The label prefers the `mul` term, so a compound with a Multilingual
        // term is labelled in it.
        assert!(
            query.contains("BIND(COALESCE(?compoundLabelMul, ?compoundLabelEn) AS ?compoundLabel)")
        );
    }
}

#[test]
fn the_taxon_filter_sits_in_the_innermost_subquery() {
    let query = compounds_by_taxon_query("Q16521");
    let ancestry = query.find("P171*").expect("the ancestry filter is present");

    // It has to precede the close of the innermost WHERE, or the endpoint
    // enriches every row and only then discards most of them.
    let after_ancestry = &query[ancestry..];
    let close_of_inner = after_ancestry.find('}').expect("the subquery closes");
    assert!(
        close_of_inner
            > after_ancestry
                .find("wd:Q16521")
                .expect("the qid is present"),
        "the filter is inside the innermost WHERE"
    );
}

#[test]
fn the_taxon_qid_is_escaped() {
    let query = compounds_by_taxon_query(r#"Q1" . OPTIONAL { ?s ?p ?o }"#);
    // A crafted QID must not be able to close the pattern and add its own.
    assert!(query.contains(r#"wd:Q1\" . OPTIONAL"#));
}

#[test]
fn no_taxon_means_no_ancestry_filter() {
    let query = all_compounds_query();
    assert!(!query.contains("P171*"));
    assert_eq!(
        subquery_depth(&query),
        subquery_depth(&compounds_by_taxon_query("Q16521"))
    );
}

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
    let query = structure_search_query("CCO", SmilesSearchType::Substructure, 0.8, Some("Q16521"));
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
fn the_count_query_drops_the_display_only_optionals() {
    let base = compounds_by_taxon_query("Q16521");
    let counts = counts_query(&base);

    assert!(counts.starts_with("PREFIX"), "the prefixes are kept");
    // The label lookup is two `FILTER(LANG(…))` passes per compound, run only to
    // derive a count. It is the single most expensive thing in the query.
    for dropped in [
        "?compoundLabelMul",
        "FILTER(LANG(",
        "OPTIONAL { ?r wdt:P1476",
        "OPTIONAL { ?c wdt:P2067",
    ] {
        assert!(
            !counts.contains(dropped),
            "{dropped} should have been stripped"
        );
    }
    // The core triples the counts actually depend on survive.
    assert!(counts.contains("wdt:P235"));
    assert!(counts.contains("COUNT(DISTINCT ?compound)"));
    assert!(counts.contains("COUNT(DISTINCT ?taxon)"));
    assert!(counts.contains("COUNT(DISTINCT ?ref_qid)"));
    assert!(counts.contains("COUNT(DISTINCT CONCAT("));
    // A separator that cannot occur inside a QID or a taxon name.
    assert!(counts.contains(r#"STR(?compound), "\u001F""#));
}

#[test]
fn the_filter_fragments_land_in_the_outermost_where_block() {
    let base = compounds_by_taxon_query("Q16521");
    let filtered = with_filters(
        &base,
        &SearchCriteria {
            mass_min: 100.0,
            mass_max: 400.0,
            year_min: 1990,
            year_max: 2010,
            ..criteria()
        },
        NOW,
    );

    let body = base_body(&base);
    let injected = filtered
        .strip_prefix(body)
        .expect("the base query is a prefix of the filtered one");

    assert!(
        injected.contains("?c wdt:P2067 ?compound_mass ."),
        "mass becomes required"
    );
    assert!(
        injected.contains("?r wdt:P577 ?ref_date ."),
        "the reference date becomes required"
    );
    assert!(
        injected.contains("FILTER(?compound_mass >= 100.000000 && ?compound_mass <= 400.000000)")
    );
    assert!(injected.contains("FILTER(YEAR(?ref_date) >= 1990 && YEAR(?ref_date) <= 2010)"));
    assert!(
        injected.trim_end().ends_with('}'),
        "and the block is closed again"
    );
}

#[test]
fn the_formula_filter_normalises_subscripts_before_comparing() {
    let base = compounds_by_taxon_query("Q16521");
    let filtered = with_filters(
        &base,
        &SearchCriteria {
            formula_enabled: true,
            // Subscripts in, ASCII compared: the input must be normalised too.
            formula_exact: "C₁₇H₁₂O₇".into(),
            f_state: ElementState::Required,
            c_min: 5,
            c_max: 20,
            ..criteria()
        },
        NOW,
    );

    assert!(filtered.contains("FILTER(BOUND(?compound_formula_raw))"));
    assert!(filtered.contains("BIND(STR(?compound_formula_raw) AS ?_formula_raw)"));
    assert!(
        filtered.contains("?_formula_tokens"),
        "the formula is tokenised"
    );
    assert!(filtered.contains("FILTER(?_count_c >= 5 && ?_count_c <= 20)"));
    assert!(
        filtered.contains("FILTER(?_count_f > 0)"),
        "a required halogen"
    );
    assert!(filtered.contains(r#"FILTER(?_formula_norm = "C17H12O7")"#));
    // The subscripts that remain are the ones the normalisation chain is written
    // from; what must not survive is a subscript inside the compared value.
    let compared = filtered
        .lines()
        .find(|l| l.contains("?_formula_norm = "))
        .expect("the exact-formula filter is present");
    assert!(
        !compared.contains('₁'),
        "the compared value is ASCII: {compared}"
    );
}

#[test]
fn every_bind_precedes_every_filter() {
    let base = compounds_by_taxon_query("Q16521");
    let filtered = with_filters(
        &base,
        &SearchCriteria {
            formula_enabled: true,
            c_min: 5,
            cl_state: ElementState::Excluded,
            ..criteria()
        },
        NOW,
    );

    let last_bind = filtered.rfind("BIND(IF(REGEX").expect("a count is bound");
    let first_filter = filtered[last_bind..]
        .find("FILTER(")
        .map(|i| last_bind + i)
        .expect("a filter follows");
    assert!(
        last_bind < first_filter,
        "a BIND is interleaved with the FILTERs"
    );
}

#[test]
fn an_excluded_halogen_becomes_an_equality_test() {
    let base = compounds_by_taxon_query("Q16521");
    let filtered = with_filters(
        &base,
        &SearchCriteria {
            formula_enabled: true,
            br_state: ElementState::Excluded,
            ..criteria()
        },
        NOW,
    );
    assert!(filtered.contains("FILTER(?_count_br = 0)"));
}

#[test]
fn no_filter_leaves_the_query_untouched() {
    let base = compounds_by_taxon_query("Q16521");
    assert_eq!(with_filters(&base, &criteria(), NOW), base);
}

#[test]
fn a_limit_is_appended_to_the_whole_query() {
    let limited = limit_query(&compounds_by_taxon_query("Q16521"), 250);
    assert!(limited.ends_with("\nLIMIT 250"));
    assert!(limited.starts_with("PREFIX"));
}

#[test]
fn the_rdf_export_rewrites_the_outer_select_as_a_construct() {
    let select = compounds_by_taxon_query("Q16521");
    let construct = construct_from_select(&select);

    assert!(!construct.contains("SELECT DISTINCT"));
    assert!(construct.contains("CONSTRUCT {"));
    assert!(construct.contains("?c wdt:P235 ?compound_inchikey ."));
    assert!(construct.contains("?statement ps:P703 ?t ;"));
    assert!(construct.contains("?r wdt:P356 ?ref_doi ."));
    // `?compound_formula` is bound only by the CONSTRUCT wrapper; the SELECT
    // projects `?compound_formula_raw`.
    assert!(construct.contains("AS ?compound_formula)"));
    assert_eq!(
        construct.matches("SELECT").count(),
        2,
        "the outer SELECT becomes CONSTRUCT; the inner two stay"
    );
    // Balanced. Three `WHERE`s is right: the CONSTRUCT's own, plus the two the
    // source query already had as nested subqueries.
    assert_eq!(construct.matches("WHERE").count(), 3);
    assert_eq!(
        construct.matches('{').count(),
        construct.matches('}').count(),
        "unbalanced braces would be a syntax error"
    );
}

#[test]
fn a_csv_or_json_export_leaves_the_select_alone() {
    let select = compounds_by_taxon_query("Q16521");
    assert_eq!(export_query(&select, false), select);
    assert_ne!(export_query(&select, true), select);
}

#[test]
fn a_taxon_lookup_is_an_exact_values_match() {
    let query = taxon_lookup_query("Gentiana lutea");
    assert!(query.contains(r#"VALUES ?taxon_name { "Gentiana lutea" }"#));
    assert!(query.contains("?taxon wdt:P225 ?taxon_name ."));
    // `VALUES` is served from an index. A `FILTER`, or an `LCASE` to match
    // case-insensitively, turns this into a scan of every label.
    assert!(!query.contains("FILTER"));
    assert!(!query.contains("LCASE"));
}

#[test]
fn a_taxon_lookup_escapes_its_literal() {
    let query = taxon_lookup_query(r#"Gentiana "lutea" \ x"#);
    assert!(query.contains(r#"VALUES ?taxon_name { "Gentiana \"lutea\" \\ x" }"#));
}
