// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//!
//! The shape of the base queries, and what they are asked for.
//!
//! Which subquery a fragment lands in, how deep the nesting goes, and what the
//! export and reference-lookup variants change. Layout may change; the query plan
//! may not.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "a test that fails on a bad fixture is reporting, not panicking"
)]
use lotus_model::ElementState;
use lotus_model::SearchCriteria;
use lotus_query::{
    Nomenclature, all_compounds_query, compounds_by_taxon_query, compounds_by_taxon_query_with,
    construct_from_select, escape_structure_literal, export_query, is_reference_lookup,
    taxon_common_name_lookup_query, taxon_lookup_query, with_filters,
};

use super::common::{NOW, carbon_ranged_filter, criteria, subquery_depth};

#[test]
fn the_compound_query_is_three_levels_deep() {
    for query in [all_compounds_query(), compounds_by_taxon_query("Q16521")] {
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
fn a_taxon_query_is_one_level_deeper_than_the_all_compounds_query() {
    assert_eq!(subquery_depth(&all_compounds_query()), 3);
    assert_eq!(
        subquery_depth(&compounds_by_taxon_query("Q16521")),
        4,
        "three, plus the synonym expansion"
    );
    assert_eq!(
        subquery_depth(&compounds_by_taxon_query_with(
            "Q16521",
            &Nomenclature::ALL_OFF
        )),
        3,
        "the expansion is the only thing that adds a level"
    );
}

/// The four relationships, and the one property deliberately left out.
///
/// `P1531` (*hybrid of*) is not in the path and must stay out: a hybrid is a
/// different organism with a parent of its own, so following it answers a
/// question about breeding history rather than about names. See
/// `lotus_model::taxon_nomenclature` for the argument in full.
#[test]
fn the_expansion_is_its_own_subquery() {
    let query = compounds_by_taxon_query("Q122679");

    let expansion = query
        .find("VALUES ?seed { wd:Q122679 }")
        .expect("the seed is a constant, not a join");
    let root_at = query
        .find("?t (wdt:P171*) ?root .")
        .expect("the ancestry filter runs off the expanded roots");
    assert!(expansion < root_at, "expansion comes first");

    // The expansion's `SELECT` closes and its own `}` follows before the
    // ancestry filter, so the endpoint evaluates the expansion against one
    // constant and hands `P171*` a handful of QIDs rather than walking a
    // combined path from the tree's root.
    let expansion_block = &query[expansion..root_at];
    assert!(
        expansion_block.contains("}\n          }"),
        "the expansion is a self-contained subquery, closing before P171*"
    );
}

/// No predicate is written into the path twice.
///
/// A repeated predicate makes the query longer and its cache key different for
/// no gain, and the closure list is the kind of thing that grows a duplicate
/// when someone adds a property that is already there.
#[test]
fn turning_one_off_leaves_the_others_alone() {
    let only_basionym_off =
        Nomenclature::ALL_ON.without(&lotus_model::taxon_nomenclature::BASIONYM);
    let query = compounds_by_taxon_query_with("Q122679", &only_basionym_off);
    for (property, prefixed) in lotus_model::taxon_nomenclature::BASIONYM.properties() {
        assert!(!query.contains(prefixed), "{property} is switched off");
    }
    for relation in [
        lotus_model::taxon_nomenclature::ACCEPTED_SYNONYM,
        lotus_model::taxon_nomenclature::PROTONYM,
        lotus_model::taxon_nomenclature::REPLACEMENT,
    ] {
        for (property, prefixed) in relation.properties() {
            assert!(
                query.contains(prefixed),
                "{property} survives an unrelated toggle"
            );
        }
    }
}

/// The criteria carry the four flags through to the query, which is the whole
/// job of `Nomenclature::from`.
#[test]
fn the_criteria_flags_reach_the_query() {
    let criteria = SearchCriteria {
        taxon_names: lotus_model::TaxonNomenclature {
            basionyms: false,
            ..lotus_model::TaxonNomenclature::ALL_ON
        },
        ..criteria()
    };
    let nomenclature = Nomenclature::from(&criteria);
    assert!(!nomenclature.follows(&lotus_model::taxon_nomenclature::BASIONYM));
    assert!(nomenclature.follows(&lotus_model::taxon_nomenclature::ACCEPTED_SYNONYM));
    assert!(nomenclature.follows(&lotus_model::taxon_nomenclature::PROTONYM));
    assert!(nomenclature.follows(&lotus_model::taxon_nomenclature::REPLACEMENT));

    let query = compounds_by_taxon_query_with("Q122679", &nomenclature);
    assert!(!query.contains("wdt:P566"), "the basionym is left out");
    assert!(query.contains("wdt:P1420"), "the rest is untouched");
}

/// A structure search is run *within* a taxon, and the taxon's closure is the
/// same set of organisms however the compound was found.
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
fn the_rdf_export_rewrites_the_outer_select_as_a_construct() {
    let select = compounds_by_taxon_query("Q16521");
    let construct = construct_from_select(&select);

    // The *outer* SELECT is the one that becomes a CONSTRUCT. The nested
    // subqueries keep their own `SELECT DISTINCT` — they are subqueries, not
    // the result shape — so the assertion is that the CONSTRUCT comes first,
    // not that the word `SELECT` is gone.
    assert!(construct.contains("CONSTRUCT {"));
    assert!(
        construct
            .find("CONSTRUCT")
            .expect("a CONSTRUCT was written")
            < construct.find("SELECT").expect("a nested SELECT survived"),
        "the outer SELECT is the one that was rewritten"
    );
    assert!(construct.contains("?c wdt:P235 ?compound_inchikey ."));
    assert!(construct.contains("?statement ps:P703 ?t ;"));
    assert!(construct.contains("?r wdt:P356 ?ref_doi ."));
    // `?compound_formula` is bound only by the CONSTRUCT wrapper; the SELECT
    // projects `?compound_formula_raw`.
    assert!(construct.contains("AS ?compound_formula)"));
    assert_eq!(
        construct.matches("SELECT").count(),
        3,
        "the outer SELECT becomes CONSTRUCT; the inner three stay"
    );
    // Balanced. Four `WHERE`s is right: the CONSTRUCT's own, plus the two the
    // source query already had as nested subqueries, plus the one the synonym
    // expansion adds.
    assert_eq!(construct.matches("WHERE").count(), 4);
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

// ── The common-name fallback ─────────────────────────────────────────────────
//
// Each assertion below is a bug that shipped. They are pinned here because the
// failure mode in every case is the same and it is invisible: the query runs,
// the endpoint is happy, and the answer is simply empty.

#[test]
fn a_common_name_lookup_walks_the_statement_path_not_the_truthy_one() {
    let query = taxon_common_name_lookup_query("bitterwort");
    // `wdt:P1843` collapses a taxon to its preferred-rank value. *Gentiana
    // lutea* carries 65 common names and `wdt:` returns one of them, so
    // `bitterwort` was unreachable. The statement path returns all 65.
    assert!(
        query.contains("?taxon p:P1843/ps:P1843 ?taxon_name ."),
        "the truthy path hides every non-preferred common name:\n{query}"
    );
    assert!(
        !query.contains("wdt:P1843"),
        "reaching for wdt: here is the bug:\n{query}"
    );
}

#[test]
fn a_common_name_lookup_compares_lexical_forms() {
    let query = taxon_common_name_lookup_query("bitterwort");
    // Every `P1843` value is language-tagged -- `bitterwort` is
    // `bitterwort@en` -- and a bare literal is not equal to a tagged one. So the
    // comparison has to go through `STR()`, or the query matches nothing at all.
    // It has to be case-insensitive too: `P1843` stores whatever a curator
    // typed, so one taxon carries `bitterwort`, `Common wormwood` and
    // `Bijvoet` side by side.
    assert!(
        query.contains(r#"FILTER(LCASE(STR(?taxon_name)) = LCASE("bitterwort"))"#),
        "tag and case both have to be discarded to match a common name:\n{query}"
    );
    assert!(query.contains(r#"BIND("common" AS ?matched_by)"#));
}

#[test]
fn the_scientific_lookup_stays_on_the_index_and_says_which_property_it_used() {
    // The fallback exists so this one need not become a scan. If it ever grows a
    // FILTER, every taxon search pays for the common-name lookup it did not need.
    let query = taxon_lookup_query("Gentiana lutea");
    assert!(
        !query.contains("P1843"),
        "the common name is a second query"
    );
    assert!(query.contains(r#"BIND("scientific" AS ?matched_by)"#));
}

#[test]
fn a_common_name_lookup_escapes_its_literal() {
    // The name goes inside a FILTER now, where a quote would end the string.
    let query = taxon_common_name_lookup_query(r#"Gentiana "lutea" \ x"#);
    assert!(query.contains(r#"LCASE("Gentiana \"lutea\" \\ x")"#));
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
