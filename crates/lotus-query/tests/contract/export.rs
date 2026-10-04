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
    Nomenclature, SELECT_COLUMNS, all_compounds_query, compound_alias_query, compound_by_qid_query,
    compound_inchikey_query, compound_label_query, compounds_by_taxon_query,
    compounds_by_taxon_query_with, construct_from_select, escape_structure_literal, export_query,
    is_reference_lookup, reference_by_doi_query, reference_by_qid_query,
    structure_compound_lookup_query, taxon_common_name_lookup_query, taxon_lookup_query,
    with_filters,
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

// ── The compound lookups ─────────────────────────────────────────────────────

#[test]
fn a_compound_lookup_never_unions_two_label_properties() {
    // The shape is forced by measurement, not taste: QLever will not push a
    // `VALUES` into both arms of a union, so it scans each property instead,
    // and scanning `rdfs:label` means 17.9M rows and a timeout. Each label route
    // is its own request, and each of them answers in about a fifth of a second.
    let label = compound_label_query("aspirin");
    let alias = compound_alias_query("aspirin");
    assert!(label.contains("?compound rdfs:label ?name ."));
    assert!(alias.contains("?compound skos:altLabel ?name ."));
    assert!(!label.contains("UNION"), "label query:\n{label}");
    assert!(!alias.contains("UNION"), "alias query:\n{alias}");
}

#[test]
fn a_compound_name_lookup_matches_a_language_tag_rather_than_discarding_it() {
    // A label *is* language-tagged, so -- the opposite of P1843 -- the tag has to
    // be part of the match. A lexical-form filter would drop it and times out
    // over 17.9M rows; enumerating the tags is what makes it index-served.
    let query = compound_label_query("aspirin");
    assert!(query.contains(r#""aspirin"@en"#));
    assert!(query.contains(r#""aspirin"@de"#));
    assert!(
        !query.contains("LCASE(STR("),
        "a lexical-form filter is the timeout:\n{query}"
    );
}

#[test]
fn a_compound_name_lookup_keeps_lexemes_out() {
    // `?compound rdfs:label "aspirin"@en` also matches three senses of the
    // English lexeme, which are not compounds.
    let query = compound_label_query("aspirin");
    assert!(
        query.contains(r#"STRSTARTS(STR(?compound), "http://www.wikidata.org/entity/Q")"#),
        "{query}"
    );
}

#[test]
fn every_lookup_brings_back_the_compounds_own_structure() {
    // An exact search does not need it, but substructure and similarity do: they
    // ask about other compounds, and the compound's own canonical SMILES is what
    // they have to hand the service. A lookup that omitted it would fall back to
    // the text the reader typed -- which, for a name, is not a structure at all.
    for query in [
        compound_by_qid_query("Q23118"),
        compound_inchikey_query("BSYNRYMUTXBXSQ-UHFFFAOYSA-N"),
        compound_label_query("aspirin"),
        compound_alias_query("aspirin"),
        structure_compound_lookup_query("C[C@H](O)CO"),
    ] {
        assert!(query.contains("wdt:P233"), "{query}");
        assert!(query.contains("canonical_smiles"), "{query}");
    }
}

#[test]
fn every_compound_lookup_returns_the_same_columns() {
    // One parser reads all of them, so the projection has to be identical. A
    // column added to one and not the others parses as an empty field on the
    // rest, which is how a resolved compound ends up with no structure to search.
    for query in [
        compound_by_qid_query("Q23118"),
        compound_inchikey_query("BSYNRYMUTXBXSQ-UHFFFAOYSA-N"),
        compound_label_query("aspirin"),
        compound_alias_query("aspirin"),
        structure_compound_lookup_query("C[C@H](O)CO"),
    ] {
        assert!(
            query.contains("?compound_qid) ?compound_label ?canonical_smiles ?matched_by"),
            "{query}"
        );
    }
}

#[test]
fn the_resolution_query_takes_no_cutoff_from_the_reader() {
    // The reader's threshold is for the search they asked for. Resolution is a
    // separate question -- "is this structure a compound you have?" -- and it is
    // asked at 1.0 whatever they set. If it borrowed their cutoff, a lenient
    // search would resolve a structure to a near-neighbour and the exact route
    // would then report that near-neighbour as the compound they named.
    //
    // The guarantee is structural: the function has no threshold parameter to
    // borrow.
    let query = structure_compound_lookup_query("C[C@H](O)CO");
    assert!(
        query.contains(r#"sachem:cutoff "1"^^xsd:double"#),
        "{query}"
    );
    assert!(
        !query.contains("similarCompoundSearch [\n        sachem:query"),
        "one search, one cutoff: {query}"
    );
}

#[test]
fn a_structure_is_resolved_at_a_cutoff_of_one_and_only_one() {
    // Also the guard on the wording above: exactly one cutoff appears, so there is
    // no second one to be influenced.
    let query = structure_compound_lookup_query("c1ccccc1");
    assert_eq!(query.matches("sachem:cutoff").count(), 1, "{query}");
}

#[test]
fn a_structure_resolution_is_bounded() {
    // A structure can match several compounds at 1.0 -- stereoisomers, salts,
    // isotopologues. The cap keeps the resolution cheap; the caller reports the
    // rest as an ambiguity rather than hiding it.
    let query = structure_compound_lookup_query("CC(=O)OC1=CC=CC=C1C(=O)O");
    assert!(query.contains("LIMIT 5"), "{query}");
}

#[test]
fn a_qid_lookup_seeds_on_the_item_and_matches_nothing() {
    // There is no property to match: the point is to confirm the item exists, so
    // the only way to fail is for the `VALUES` row to name nothing.
    let query = compound_by_qid_query("Q23118");
    assert!(query.contains("VALUES ?compound { wd:Q23118 }"), "{query}");
    assert!(!query.contains("wdt:P235"), "{query}");
    assert!(!query.contains("rdfs:label ?name"), "{query}");
}

#[test]
fn a_lowercase_qid_is_accepted_the_way_the_taxon_field_accepts_one() {
    // A reader who types `q23118` means Q23118. The prefix is part of the local
    // name, so dropping it would seed the query with a bare number and match
    // nothing at all.
    assert!(compound_by_qid_query("q23118").contains("wd:Q23118"));
}

#[test]
fn a_compound_lookup_reports_the_bare_qid_the_parser_expects() {
    let query = compound_label_query("aspirin");
    assert!(
        query.contains("AS ?compound_qid"),
        "the CSV must carry a bare Q, not a URI:\n{query}"
    );
}

#[test]
fn a_compound_name_lookup_escapes_its_literal() {
    let query = compound_label_query(r#"Gentiana "lutea" \ x"#);
    assert!(query.contains(r#""Gentiana \"lutea\" \\ x"@en"#), "{query}");
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

// ── The reference lookup queries ──────────────────────────────────────────────

#[test]
fn a_doi_lookup_asks_p356_for_the_uppercased_doi() {
    // Every assertion here is a query that would run, be answered, and return
    // nothing. That is the failure mode of all of them, so each has to be pinned.
    let query = reference_by_doi_query("10.1002/andp.18280880206");
    // Wikidata stores DOIs uppercased and `QLever` returns nothing for a
    // lowercased one, so this is the whole reason the lookup finds anything.
    assert!(
        query.contains(r#"?ref wdt:P356 "10.1002/ANDP.18280880206""#),
        "{query}"
    );
    // The three prefixes a reader pastes from a paper, an email and a reference
    // manager, none of which Wikidata has ever heard of.
    for pasted in [
        "doi:10.1002/andp.18280880206",
        "https://doi.org/10.1002/andp.18280880206",
        "http://dx.doi.org/10.1002/andp.18280880206",
    ] {
        assert_eq!(reference_by_doi_query(pasted), query, "{pasted}");
    }
}

#[test]
fn a_doi_lookup_escapes_the_doi_rather_than_interpolating_it() {
    // A DOI is external input. A quote or a backslash in it ends the literal and
    // lets the rest of the string become query text.
    let query = reference_by_doi_query(r#"10.1000/a". ?ref ?o ."#);
    assert!(
        !query.contains(r#"P356 "10.1000/A". ?ref ?o .""#),
        "{query}"
    );
    assert!(query.contains(r#"\""#), "{query}");
}

#[test]
fn a_qid_lookup_is_a_values_and_nothing_to_match() {
    // `VALUES` is served from an index and costs about what a compound lookup by
    // QID does. A `FILTER`, or an `LCASE` to match case-insensitively, turns this
    // into a scan of every item.
    let query = reference_by_qid_query("Q23118");
    assert!(query.contains("VALUES ?ref { wd:Q23118 }"), "{query}");
    assert!(!query.contains("FILTER"), "{query}");
    assert!(!query.contains("LCASE"), "{query}");
    // The `Q` goes in with the number: `wd:` takes a local name, so `wd:Q23118`
    // is the item and `wd:23118` is a name nothing is bound to.
    assert!(!query.contains("wd:23118"), "{query}");
}

#[test]
fn a_qid_lookup_accepts_a_lowercase_q_and_surrounding_space() {
    // A reader who types `q23118` means Q23118, the same as in the taxon field,
    // and a pasted value carries whatever space a line break left behind.
    assert_eq!(
        reference_by_qid_query("q23118"),
        reference_by_qid_query("Q23118")
    );
    assert_eq!(
        reference_by_qid_query("  Q23118  "),
        reference_by_qid_query("Q23118")
    );
}

// ── Reference-lookup detection ────────────────────────────────────────────────

#[test]
fn a_doi_lookup_is_recognised_but_a_qid_lookup_is_not() {
    // The distinction is what routes a failed DOI to the WDQS scholarly subgraph,
    // the one service that answers `P356` at all. A `VALUES` is answered anywhere,
    // so routing it there would buy nothing.
    assert!(is_reference_lookup(&reference_by_doi_query(
        "10.1002/andp.18280880206"
    )));
    assert!(!is_reference_lookup(&reference_by_qid_query("Q23118")));
}

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

/// The nothing-provided query and the `*` query are different requests.
///
/// They were the same query, which meant submitting the form with an empty taxon box
/// answered "what has been reported and where" -- a narrower question than the one that
/// was asked -- without saying so.
#[test]
fn nothing_provided_includes_compounds_with_no_occurrence_and_star_does_not() {
    let with_occurrence = lotus_query::all_compounds_query();
    let everything = lotus_query::all_compounds_including_untaxonomised_query();

    // `*` is the explicit request for what has been reported, so it requires the
    // statement that says so.
    assert!(
        with_occurrence.contains("?c p:P703 ?statement"),
        "* means every compound with an occurrence, so P703 is required"
    );
    // Structural, not a count: this base query legitimately contains other
    // `OPTIONAL`s for the reference metadata, so "does it mention OPTIONAL" says
    // nothing. What distinguishes the two is whether the *occurrence* sits inside one.
    assert!(
        !occurrence_is_optional(&with_occurrence),
        "* must not make the occurrence optional, or it is the other query"
    );

    // Nothing provided is not "report only what has an occurrence".
    assert!(
        occurrence_is_optional(&everything),
        "an empty taxon box must not silently exclude compounds with no organism"
    );
    assert!(
        everything.contains("?c p:P703 ?statement"),
        "the occurrence is optional, not removed: the columns still have to hold one"
    );
    assert_ne!(
        everything, with_occurrence,
        "the two queries converged, so one of the two cases cannot be answered"
    );
}

/// The occurrence is optional as one block, not triple by triple.
#[test]
fn the_optional_occurrence_keeps_its_hops_together() {
    let everything = lotus_query::all_compounds_including_untaxonomised_query();
    // Optional per triple would emit a row with a taxon but no reference, which the
    // result store cannot represent as an occurrence. Both hops have to be inside the
    // same block as the statement that starts it.
    assert!(everything.contains("?statement ps:P703 ?t"));
    assert!(everything.contains("?ref pr:P248 ?r"));

    let block = occurrence_block(&everything);
    assert!(
        block.contains("ps:P703") && block.contains("pr:P248"),
        "the taxon and the reference must be inside the same OPTIONAL as the statement"
    );
}

/// The `OPTIONAL` block the occurrence statement sits in, or `""` if it is not in one.
fn occurrence_block(query: &str) -> &str {
    let occurrence = query.find("?c p:P703 ?statement").unwrap_or(0);
    let open = query[..occurrence].rfind("OPTIONAL {").unwrap_or(0);
    // An OPTIONAL that opens after the occurrence cannot contain it.
    if open == 0 && !query[..occurrence].contains("OPTIONAL {") {
        return "";
    }
    query[open..].split('}').next().unwrap_or("")
}

/// Whether the occurrence statement is inside an `OPTIONAL`.
fn occurrence_is_optional(query: &str) -> bool {
    !occurrence_block(query).is_empty()
}

/// The variable names `select_clause` actually projects, in order.
///
/// Read out of the query text rather than compared against a stored copy of it,
/// because a stored copy is a second thing to forget. Each projected item is one
/// line of the `SELECT DISTINCT` block, and the variable is the last `?name` on
/// it -- which covers both a bare `?compoundLabel` and an aliased
/// `(xsd:integer(STRAFTER(STR(?c), "Q")) AS ?compound)`.
fn projected_names(query: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_projection = false;
    for line in query.lines() {
        let trimmed = line.trim();
        if trimmed == "SELECT DISTINCT" {
            in_projection = true;
            continue;
        }
        if !in_projection {
            continue;
        }
        if trimmed.starts_with("WHERE") || trimmed.is_empty() {
            break;
        }
        let last = trimmed
            .rsplit('?')
            .next()
            .expect("a projected item names a variable")
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .next()
            .expect("a variable has a name after the ?")
            .to_string();
        names.push(last);
    }
    names
}

#[test]
fn the_projection_is_exactly_the_columns_the_parser_is_told_about() {
    // The parser finds every column by name, so a name projected but not known to
    // it is not a build error and not a parse error: it is a column that arrives
    // empty, for every row, forever. `SELECT_COLUMNS` is the list both sides are
    // held to, and this is what keeps the projection text and that list the same
    // list.
    for (name, query) in [
        ("taxon", compounds_by_taxon_query("Q16521")),
        ("all compounds", all_compounds_query()),
    ] {
        assert_eq!(
            projected_names(&query),
            SELECT_COLUMNS,
            "the {name} query projects a different set of columns than \
             SELECT_COLUMNS records. Change both together: the list is what the \
             parser and the export fidelity test are checked against."
        );
    }
}
