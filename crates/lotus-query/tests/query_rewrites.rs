// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The three rewrites a caller can hand a string that is not a query.
//!
//! `counts_query`, `limit_query` and `construct_from_select` all take a base query
//! and reshape it. Each has a branch for a base that does not have the shape it
//! expects — no `SELECT`, no `WHERE`, no closing brace, or `usize::MAX` meaning
//! "no limit" — and none of those branches had a test. They are the branches a
//! caller reaches by passing something unexpected, so they are the ones where a
//! silent pass-through returns a query the endpoint answers with something
//! other than what was asked for.

// The panic lints exist to keep library code free of panics on external input.
// A test that fails on a bad argument is reporting, not panicking.
#![allow(unused_crate_dependencies)]
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use lotus_query::{construct_from_select, counts_query, escape_sparql_string, limit_query};

/// A minimal well-formed base query: prefixes, a `SELECT`, a `WHERE`, one brace.
const BASE: &str = "PREFIX wdt: <http://www.wikidata.org/prop/direct/>\n\
                    SELECT ?compound WHERE {\n  ?compound wdt:P31 ?type .\n}\n";

#[test]
fn a_base_without_a_select_is_returned_untouched() {
    // Both rewriters look for `SELECT` to split on. Without one there is nothing
    // to split, so the caller gets back what it passed -- not a query with the
    // prefixes spliced into the wrong place.
    for base in ["", "ASK { ?a ?b ?c }", "# a comment\n"] {
        assert_eq!(counts_query(base), base, "counts_query mangled {base:?}");
        assert_eq!(
            construct_from_select(base),
            base,
            "construct_from_select mangled {base:?}"
        );
    }
}

#[test]
fn a_select_without_a_where_is_not_rewritten_into_a_construct() {
    // `construct_from_select` needs the trailing `WHERE` block to splice the
    // provenance graph into. A `SELECT` with no `WHERE` has no block, and the
    // output has to be the input rather than a CONSTRUCT with an empty graph.
    let base = "SELECT ?compound WHERE";
    assert_eq!(construct_from_select(base), base);
}

#[test]
fn a_where_block_with_no_closing_brace_is_not_rewritten() {
    // Both rewriters strip the final `}` before splicing and re-emit it. With no
    // final brace to strip there is nothing to rebalance, so passing the input
    // through is the only answer that does not invent a brace.
    let base = "SELECT ?compound WHERE {\n  ?compound ?p ?o\n";
    assert_eq!(construct_from_select(base), base);
}

#[test]
fn a_limit_of_usize_max_means_no_limit_and_emits_no_number() {
    // `usize::MAX` is how a caller says "no limit". Emitting
    // `LIMIT 18446744073709551615` instead asks the endpoint for a bound it
    // cannot honour, and it shows up in the query `--explain` prints.
    let unlimited = limit_query(BASE, usize::MAX);
    assert_eq!(
        unlimited,
        BASE.trim_end(),
        "the base must come back unchanged"
    );
    assert!(
        !unlimited.contains("LIMIT"),
        "an explicit no-limit must not emit one: {unlimited:?}"
    );

    // And the other side of the rule, so the two cannot be swapped: a real limit
    // is emitted, and trailing whitespace does not survive in front of it.
    assert_eq!(
        limit_query(BASE, 25),
        format!("{}\nLIMIT 25", BASE.trim_end())
    );
}

#[test]
fn every_character_that_would_end_a_string_literal_is_escaped() {
    // A user-supplied taxon name goes into a SPARQL string literal. A bare
    // newline, carriage return or tab is a literal terminator to some parsers and
    // a syntax error to the rest, and a quote ends the literal outright -- which
    // is how a value typed into the taxon box becomes part of the query rather
    // than an argument to it.
    let cases = [
        ("plain", "Gentiana", "Gentiana"),
        ("backslash", r"a\b", r"a\\b"),
        ("double quote", "a\"b", r#"a\"b"#),
        ("single quote", "a'b", "a'b"),
        ("newline", "a\nb", r"a\nb"),
        ("carriage return", "a\rb", r"a\rb"),
        ("tab", "a\tb", r"a\tb"),
        ("all three together", "\"\\\n\r\t", r#"\"\\\n\r\t"#),
    ];
    for (what, input, expected) in cases {
        assert_eq!(
            escape_sparql_string(input),
            expected,
            "{what} is not escaped the way a SPARQL literal needs"
        );
    }
}

#[test]
fn an_injected_clause_cannot_leave_the_literal() {
    // The property that matters, stated as one assertion rather than five
    // escapes: whatever goes in, what comes out contains exactly one pair of
    // unescaped delimiters, so there is nothing for a value to close.
    for input in [
        "Gentiana\" . ?x wdt:P31 ?evil . FILTER(",
        "Gentiana\n}\nLIMIT 1",
        "Gentiana\\\" } DROP",
    ] {
        let escaped = escape_sparql_string(input);
        // Every quote that survives is a preceded one. That is the whole
        // property: a value cannot contribute a delimiter of its own, so the
        // only quote closing the literal is the one the template opened.
        let unescaped = escaped
            .char_indices()
            .filter(|(i, c)| *c == '"' && !escaped[..*i].ends_with('\\'))
            .count();
        assert_eq!(
            unescaped, 0,
            "{unescaped} quotes survive unescaped, so the value can close the literal: \
             {escaped:?}"
        );
        assert!(
            !escaped.contains('\n') && !escaped.contains('\r'),
            "a raw line break survives into the query: {escaped:?}"
        );
    }
}
