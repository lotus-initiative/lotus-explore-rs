// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Structural rules every generated query must satisfy, checked over *all* of them.
//!
//! The other contract modules assert what a given query should contain. This one
//! asserts properties of every query the crate can generate, because the defect
//! that actually shipped was not specific to one builder.
//!
//! ## Why this exists
//!
//! `FILTER(BOUND(?ref))` was added inside three `OPTIONAL`s to stop an unbound
//! `?r` becoming a fresh variable. It blanked every reference title, DOI and date
//! in the product, and nothing caught it, because:
//!
//! - the query still ran, and returned the same *number* of rows;
//! - the affected columns are optional, so an empty cell is indistinguishable
//!   from a reference that genuinely has no metadata;
//! - the commit that introduced it was fixing a real 54,116,026-row scan, and
//!   verified its change by row count.
//!
//! The cause is a scoping rule that is easy to state and easy to forget: **a
//! `FILTER` inside an `OPTIONAL` is evaluated against that `OPTIONAL`'s own
//! solutions, before the join with the left side.** So a variable bound only
//! outside is unbound inside, `BOUND` of it is false, and the `OPTIONAL` rejects
//! everything. Measured: 87 rows carried a title before the guard, 0 after.
//!
//! Nothing about that is visible in a row count, so it is checked here instead.
//! These are lexical properties of the query text, which is the only place the
//! rule is observable without an endpoint.

use lotus_model::SmilesSearchType;
use lotus_query::{
    Nomenclature, all_compounds_including_untaxonomised_query, all_compounds_query,
    compounds_by_taxon_query, exact_compound_query, structure_search_query,
};

/// Every query shape the crate can build, with a label for the failure message.
///
/// The matrix deliberately spans the axes that change the query's structure:
/// taxon present or absent (which decides whether the occurrence block is
/// required or optional), taxon concrete or wildcard, structure present or
/// absent, and the reference layer on or off. A rule checked against one shape
/// is a rule that only holds for that shape.
fn every_shape() -> Vec<(String, String)> {
    let nomen = Nomenclature::default();
    vec![
        ("taxon, concrete".into(), compounds_by_taxon_query("Q21754")),
        ("taxon, wildcard".into(), all_compounds_query()),
        (
            "no taxon, untaxonomised".into(),
            all_compounds_including_untaxonomised_query(),
        ),
        (
            "taxon + structure".into(),
            structure_search_query("c1ccccc1", SmilesSearchType::Similarity, 0.4, None),
        ),
        (
            "structure only".into(),
            structure_search_query("c1ccccc1", SmilesSearchType::Substructure, 0.4, None),
        ),
        (
            "exact compound + taxon".into(),
            exact_compound_query("Q153", Some(("Q21754", &nomen))),
        ),
        ("exact compound".into(), exact_compound_query("Q153", None)),
    ]
}

/// Lexically-balanced `{...}` group starting at the `{` at or after `at`.
fn group_at(query: &str, at: usize) -> Option<&str> {
    let bytes = query.as_bytes();
    let mut depth = 0usize;
    for (i, b) in bytes.iter().enumerate().skip(at) {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&query[at..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Every `OPTIONAL { ... }` group in the query.
fn optionals(query: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut rest = query;
    while let Some(at) = rest.find("OPTIONAL") {
        let after = &rest[at..];
        if let Some(brace) = after.find('{')
            && let Some(group) = group_at(after, brace)
        {
            found.push(group);
        }
        rest = &rest[at + "OPTIONAL".len()..];
    }
    found
}

/// Variables a `FILTER(BOUND(?x))` inside `group` refers to.
fn bound_tests(group: &str) -> Vec<String> {
    let mut vars = Vec::new();
    let mut rest = group;
    while let Some(at) = rest.find("BOUND(") {
        let tail = &rest[at + "BOUND(".len()..];
        if let Some(close) = tail.find(')') {
            let name = tail[..close].trim();
            if name.starts_with('?') {
                vars.push(name.to_string());
            }
        }
        rest = &rest[at + "BOUND(".len()..];
    }
    vars
}

/// True when `var` is bound by a triple pattern *within* `group`.
///
/// A `?subject predicate ?object` pattern. Deliberately conservative: a variable
/// only counts as bound if it appears in subject or object position of a pattern
/// inside the group, which is the only case where the value exists when the
/// `FILTER` is evaluated.
fn bound_within(group: &str, var: &str) -> bool {
    for line in group.lines() {
        let line = line.trim();
        // Skip the FILTER itself and anything that is not a triple pattern.
        if line.is_empty() || line.starts_with("FILTER") || line.starts_with('}') {
            continue;
        }
        // Strip a trailing `.` and split on whitespace. Enough for the shapes
        // this crate emits, and a false negative only makes the rule stricter.
        let pattern = line.trim_end_matches(['.', '}', ' ']);
        let mut parts = pattern.split_whitespace();
        let (Some(subject), Some(_predicate)) = (parts.next(), parts.next()) else {
            continue;
        };
        if subject == var {
            return true;
        }
        if parts.any(|object| object == var) {
            return true;
        }
    }
    false
}

/// A `BOUND` inside an `OPTIONAL` must be about a variable that group can see.
///
/// This is the rule that was broken. Asserted over every shape so the next
/// builder to reach for `FILTER(BOUND(...))` inside an `OPTIONAL` finds out here.
#[test]
fn no_optional_guards_on_a_variable_it_cannot_see() {
    for (label, query) in every_shape() {
        for group in optionals(&query) {
            for var in bound_tests(group) {
                assert!(
                    bound_within(group, &var),
                    "{label}: OPTIONAL {{ ... FILTER(BOUND({var})) }} tests a variable \
                     the OPTIONAL cannot bind. A FILTER inside an OPTIONAL is evaluated \
                     against that OPTIONAL's own solutions, before the join, so {var} is \
                     unbound there and the guard rejects every row -- silently emptying \
                     the column rather than erroring. Bind it in the enclosing scope and \
                     pass it through (as ?_ref_source does), or move the guard out."
                );
            }
        }
    }
}

/// The reference metadata must reach the endpoint through the sentinel.
///
/// Without `?_ref_source` an unbound `?r` becomes a fresh variable and the
/// `OPTIONAL` enumerates every reference in Wikidata. That was measured at
/// 54,116,026 rows for one compound with no occurrences.
#[test]
fn reference_metadata_goes_through_the_sentinel() {
    for (label, query) in every_shape() {
        if !query.contains("wdt:P1476") {
            continue;
        }
        assert!(
            query.contains("BIND(COALESCE(?r, wd:Q0) AS ?_ref_source)"),
            "{label}: reference metadata without the sentinel"
        );
        for predicate in ["P1476", "P356", "P577"] {
            assert!(
                !query.contains(&format!("OPTIONAL {{ ?r wdt:{predicate}")),
                "{label}: {predicate} still reads ?r directly, so an unbound ?r is fresh"
            );
        }
    }
}

/// The count query must not pay for what it does not report.
///
/// `counts_query` strips the metadata block by string match on the constant. If a
/// future edit moves a pattern outside that constant, the count query keeps it and
/// a `COUNT` pays for a scan it cannot afford -- the exact failure the sentinel
/// exists to prevent.
#[test]
fn the_count_query_carries_no_reference_or_compound_metadata() {
    for (label, query) in every_shape() {
        let counts = lotus_query::counts_query(&query);
        for needle in ["P1476", "P356", "P577", "_ref_source", "P2017", "P2067"] {
            assert!(
                !counts.contains(needle),
                "{label}: counts_query kept `{needle}`; the strip is a string match, so \
                 anything guarding or fetching that column has to live inside the constant"
            );
        }
    }
}
