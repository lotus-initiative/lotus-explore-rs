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

use lotus_model::element_max;
use lotus_model::{ElementState, SearchCriteria, SmilesSearchType};
use lotus_query::{
    all_compounds_query, compounds_by_taxon_query, construct_from_select, counts_query,
    escape_structure_literal, export_query, is_reference_lookup, limit_query,
    structure_search_query, taxon_lookup_query, with_filters,
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

// ── Formula element ranges ────────────────────────────────────────────────────

/// The formula filter emits a `FILTER(?_count_c >= … && <= …)` only when the
/// criterion actually constrains carbon. When it does not, the filter is noise
/// that still costs a cache key, because a query's bytes are what the cache key
/// and a shared link are derived from.
fn carbon_ranged_filter(c_min: u16, c_max: u16) -> Option<String> {
    let query = with_filters(
        &compounds_by_taxon_query("Q16521"),
        &SearchCriteria {
            formula_enabled: true,
            // Something has to make the formula filter run at all, or the element
            // loop is never reached and the guard cannot be distinguished from
            // any other spelling of it. An exact formula does that without
            // touching the carbon bounds under test.
            formula_exact: "CCO".into(),
            c_min,
            c_max,
            ..criteria()
        },
        NOW,
    );
    query
        .lines()
        .find(|line| line.contains("?_count_c >="))
        .map(str::trim)
        .map(str::to_string)
}

#[test]
fn an_element_pinned_to_its_full_range_gets_no_filter() {
    // `min > 0` is false, `max < default_max` is false, so nothing is emitted.
    // This is the case every boundary mutant in the guard breaks: `>` to `==`,
    // `>` to `>=`, `<` to `==` and `<` to `<=` each make this emit a filter
    // against a criterion that constrains nothing.
    assert_eq!(
        carbon_ranged_filter(0, element_max::C),
        None,
        "a full-range carbon criterion is not a constraint, so it gets no filter"
    );
}

#[test]
fn a_max_above_the_natural_maximum_gets_no_filter() {
    // `max < default_max` is false when the max is *larger* than the element's
    // natural maximum, and that is correct: nothing has 600 carbons in this
    // domain, so the bound is vacuous. `max > default_max` would emit a filter
    // here, and `c_max` is user input, so the bound is not always well-formed.
    assert_eq!(
        carbon_ranged_filter(0, element_max::C + 88),
        None,
        "a max beyond the natural maximum is vacuous, not a constraint"
    );
}

#[test]
fn either_bound_alone_is_enough_to_emit_the_filter() {
    // The guard is `min > 0 || max < default_max`, and each half is reachable on
    // its own: a lower bound with the default max, and an upper bound with no
    // lower. `||` mutated to `&&` drops the filter in both of these, which is a
    // silently wrong query rather than a failing one -- it returns the
    // unfiltered result set.
    assert_eq!(
        carbon_ranged_filter(5, element_max::C).as_deref(),
        Some("FILTER(?_count_c >= 5 && ?_count_c <= 512)"),
        "a lower bound alone constrains carbon"
    );
    assert_eq!(
        carbon_ranged_filter(0, 3).as_deref(),
        Some("FILTER(?_count_c >= 0 && ?_count_c <= 3)"),
        "an upper bound alone constrains carbon"
    );
}

#[test]
fn both_bounds_together_emit_both_ends() {
    assert_eq!(
        carbon_ranged_filter(2, 7).as_deref(),
        Some("FILTER(?_count_c >= 2 && ?_count_c <= 7)")
    );
}

// ── Reference-lookup detection ────────────────────────────────────────────────

#[test]
fn a_bare_doi_lookup_is_recognised() {
    assert!(is_reference_lookup(
        "SELECT ?ref WHERE { ?compound wdt:P356 ?ref . }"
    ));
}

#[test]
fn a_reference_query_is_still_a_lookup_without_a_service_or_optional() {
    // Each condition is removed on its own, and each removal has to change the
    // answer. `!query.contains("OPTIONAL")` mutated to `query.contains` is the
    // one that would pass if every test used a query with an `OPTIONAL` in it.
    assert!(!is_reference_lookup(
        "SELECT ?item WHERE { ?item wdt:P356 ?ref . }"
    ));
    assert!(!is_reference_lookup(
        "SELECT ?ref WHERE { ?compound wdt:P31 ?ref . }"
    ));
    assert!(!is_reference_lookup(
        "SELECT ?ref WHERE { SERVICE wikibase:label { } ?c wdt:P356 ?ref . }"
    ));
    assert!(
        !is_reference_lookup("SELECT ?ref WHERE { OPTIONAL { ?c wdt:P356 ?ref . } }"),
        "an OPTIONAL DOI is a join, not a lookup"
    );
}

#[test]
fn only_the_selection_shape_makes_it_a_lookup() {
    // The `&&` between the two `contains` calls is the one that is easiest to
    // break unnoticed, because a query that satisfies the first but not the
    // second is unusual to write by hand. A real structure search does exactly
    // that: it selects `?ref` and matches `wdt:P356`, but opens differently.
    assert!(!is_reference_lookup(
        "PREFIX wd: <x> ?compound wdt:P356 ?ref ."
    ));
}

// ── Structure literals ───────────────────────────────────────────────────────

/// A molfile in one line: classified as a molfile by its terminator, but with no
/// newline of its own.
const ONE_LINE_MOLFILE: &str = "benzene V2000 M  END";

#[test]
fn a_molfile_is_triple_quoted_even_on_one_line() {
    // `is_molfile || body.contains('\n')`: this is the case where the left side
    // holds and the right does not, so `||` mutated to `&&` double-quotes a
    // molfile. Wikidata rejects that with a parse error about the structure,
    // which is a far worse failure than the query being wrong.
    assert_eq!(
        escape_structure_literal(ONE_LINE_MOLFILE),
        format!("'''{ONE_LINE_MOLFILE}'''")
    );
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

/// Every variable a `FILTER` or `BIND` in the counts query names.
///
/// Every such variable has to be bound before it is used. The ones
/// `counts_query` can unbind are the filter machinery's, so this scans only
/// those two forms and takes every `?name` inside them -- the formula filter
/// alone names seven.
fn referenced_variables(query: &str) -> Vec<String> {
    let mut vars = Vec::new();
    for line in query.lines() {
        let t = line.trim_start();
        if !(t.starts_with("FILTER(") || t.starts_with("BIND(")) {
            continue;
        }
        for part in t.split('?').skip(1) {
            let name: String = part
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            // A `?` with no usable name after it is a regex quantifier or
            // metacharacter, not a variable.
            if !name.is_empty() && !vars.contains(&name) {
                vars.push(name);
            }
        }
    }
    vars
}

/// Whether `?var` is bound by something in `query`.
fn bound_in(query: &str, var: &str) -> bool {
    // A `BIND(… AS ?var)` defines the variable it names.
    if query.contains(&format!("AS ?{var}")) {
        return true;
    }
    // Otherwise it has to come out of a triple pattern, which this crate always
    // writes with the variable last: `?c wdt:P274 ?compound_formula_raw .`
    query
        .lines()
        .any(|line| line.trim_end().ends_with(&format!("?{var} .")))
}

/// The variables each filter reads still have their bindings in the counts query.
///
/// `counts_query` deletes two blocks of query text and keeps the filters. A filter
/// reading a variable bound only inside a deleted block survives as a `FILTER`
/// over an unbound variable, matches nothing, and makes every count on the page
/// read zero -- with no error anywhere, because a `FILTER` that rejects every row
/// is a successful query returning no rows.
///
/// That is not hypothetical: the formula filter read `?compound_formula_raw`,
/// bound only inside `COMPOUND_PROPERTIES`, so every stats card read zero for any
/// search with a formula filter on. This asserts the pairing for every filter
/// this crate can build.
#[test]
fn every_filter_something_counts_query_keeps_still_has_its_binding() {
    let filters: [(&str, SearchCriteria); 4] = [
        (
            "mass",
            SearchCriteria {
                mass_min: 100.0,
                mass_max: 400.0,
                ..criteria()
            },
        ),
        (
            "year",
            SearchCriteria {
                year_min: 1990,
                ..criteria()
            },
        ),
        (
            "formula halogen",
            SearchCriteria {
                formula_enabled: true,
                cl_state: ElementState::Required,
                ..criteria()
            },
        ),
        (
            "formula element range",
            SearchCriteria {
                formula_enabled: true,
                n_min: 2,
                n_max: 5,
                ..criteria()
            },
        ),
    ];

    for (name, criteria) in filters {
        let counts = counts_query(&with_filters(
            &compounds_by_taxon_query("Q16521"),
            &criteria,
            NOW,
        ));

        assert!(
            counts.contains("FILTER("),
            "{name}: the counts query dropped every filter"
        );

        let vars = referenced_variables(&counts);
        assert!(!vars.is_empty(), "{name}: no filter variables were found");

        for var in &vars {
            assert!(
                bound_in(&counts, var),
                "{name}: the counts query uses ?{var} but nothing binds it, so \
                 every row is filtered out and every count reads zero"
            );
        }
    }
}
