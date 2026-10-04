// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//!
//! Filter injection and the count query.
//!
//! The fragments land in the outermost `WHERE` block, a filter that binds a
//! variable has to make that variable required, and the count query drops the
//! `OPTIONAL`s the display query keeps.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "a test that fails on a bad fixture is reporting, not panicking"
)]
use lotus_model::ElementState;
use lotus_model::SearchCriteria;
use lotus_model::element_max;
use lotus_query::{
    Nomenclature, compounds_by_taxon_query, compounds_by_taxon_query_with, counts_query,
    limit_query, taxon_lookup_query, with_filters,
};

use super::common::{NOW, base_body, carbon_ranged_filter, criteria};

#[test]
fn the_taxon_filter_sits_in_the_innermost_subquery() {
    for query in [
        compounds_by_taxon_query("Q16521"),
        compounds_by_taxon_query_with("Q16521", &Nomenclature::ALL_OFF),
    ] {
        let ancestry = query.find("P171*").expect("the ancestry filter is present");

        // It has to come after the innermost `SELECT` — the one that projects
        // only what the occurrence asserts — or the endpoint enriches every
        // row and only then discards most of them. Which is the load-bearing
        // question whether the QID is a seed, a filter target, or both.
        let innermost_select = query[..ancestry]
            .rfind("SELECT")
            .expect("the innermost subquery opens before the filter");
        let between = &query[innermost_select..ancestry];
        assert!(
            !between.contains("?ref_title") && !between.contains("?compoundLabelMul"),
            "the display OPTIONAL(s) are not in the innermost projection"
        );
    }
}

/// The `SELECT` plus the two subqueries the query has always nested. A taxon
/// query with synonyms adds exactly one more, for the expansion — and that is
/// the only thing that adds a level.
#[test]
fn the_taxon_qid_is_escaped() {
    for query in [
        compounds_by_taxon_query(r#"Q1" . OPTIONAL { ?s ?p ?o }"#),
        compounds_by_taxon_query_with(r#"Q1" . OPTIONAL { ?s ?p ?o }"#, &Nomenclature::ALL_ON),
    ] {
        // A crafted QID must not be able to close the pattern and add its own.
        assert!(query.contains(r#"wd:Q1\" . OPTIONAL"#));
    }
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
fn a_taxon_lookup_escapes_its_literal() {
    let query = taxon_lookup_query(r#"Gentiana "lutea" \ x"#);
    assert!(query.contains(r#"VALUES ?taxon_name { "Gentiana \"lutea\" \\ x" }"#));
}

// ── Formula element ranges ────────────────────────────────────────────────────

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

// ── The reference constraint ───────────────────────────────────────────────

#[test]
fn a_reference_constraint_binds_the_referenced_item() {
    // `?r` is the reference item, the same variable the year filter already binds,
    // so the constraint is a `VALUES` on it rather than a filter over a projected
    // value: a row with no reference has to fail the join, not compare against an
    // unbound variable and be kept.
    let criteria = SearchCriteria {
        reference: "Q34460861".to_string(),
        ..criteria()
    };
    let base = compounds_by_taxon_query("Q158572");
    let filtered = with_filters(&base, &criteria, NOW);
    assert!(
        filtered.contains("VALUES ?r { wd:Q34460861 }"),
        "{filtered}"
    );
}

#[test]
fn no_reference_constraint_writes_nothing() {
    let criteria = SearchCriteria::up_to_year(2026);
    let base = compounds_by_taxon_query("Q158572");
    let filtered = with_filters(&base, &criteria, NOW);
    assert!(!filtered.contains("VALUES ?r"), "{filtered}");
}

#[test]
fn a_reference_constraint_composes_with_the_year_filter() {
    // Both bind `?r` and both must survive: the year one adds `?r wdt:P577`, and
    // the two are separate things the reader asked for.
    let criteria = SearchCriteria {
        reference: "Q34460861".to_string(),
        year_min: 2020,
        year_max: 2021,
        ..criteria()
    };
    let base = compounds_by_taxon_query("Q158572");
    let filtered = with_filters(&base, &criteria, NOW);
    assert!(
        filtered.contains("VALUES ?r { wd:Q34460861 }"),
        "{filtered}"
    );
    assert!(filtered.contains("?r wdt:P577 ?ref_date"), "{filtered}");
    assert!(filtered.contains("FILTER(YEAR(?ref_date)"), "{filtered}");
}

/// `([A-Z])` splits every capital, so `C6H5COOH` tokenises to `|C6|H5|C|O|O|H`.
/// A greedy `\|C([0-9]*)` then matches the *trailing bare* `|C`, captures the
/// empty string, and the count collapses to 1 -- benzoic acid read as one carbon.
/// Measured against real Wikidata formulas, this miscounted `C₂H₅OOCH`.
///
/// The count must come from a token that actually carries digits. Pinned here
/// because the greedy form is shorter and looks like a simplification.
#[test]
fn the_element_count_prefers_a_token_that_carries_digits() {
    let base = compounds_by_taxon_query("Q16521");
    let filtered = with_filters(
        &base,
        &SearchCriteria {
            formula_enabled: true,
            c_min: 5,
            c_max: 20,
            ..criteria()
        },
        NOW,
    );

    assert!(
        filtered.contains(r#"REGEX(?_formula_tokens, "\\|C([0-9]+)(\\||$)")"#),
        "a digit-bearing token is what the count is read from: {filtered}"
    );
    assert!(
        filtered.contains(r#"REPLACE(?_formula_tokens, "^.*?\\|C([0-9]+)(\\||$).*", "$1")"#),
        "and it is anchored and lazy, so it stops at the first such token \
         rather than running on to the last |C of any kind: {filtered}"
    );
}
