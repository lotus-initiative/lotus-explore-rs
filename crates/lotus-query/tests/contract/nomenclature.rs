// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//!
//! The nomenclatural relationships a taxon search follows.
//!
//! Each of the four is a distinct relationship a reader can turn on or off, and
//! each contributes its own properties to the closure. The tests here are what
//! would fail if they were collapsed back into one "synonyms" switch: turning off
//! the basionym should not lose the replacement name with it.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "a test that fails on a bad fixture is reporting, not panicking"
)]
use lotus_model::SmilesSearchType;
use lotus_query::{
    Nomenclature, all_compounds_query, compounds_by_taxon_query, compounds_by_taxon_query_with,
    structure_search_query_with,
};

use super::common::{criteria, subquery_depth};

#[test]
fn the_closure_covers_every_nomenclatural_property() {
    let query = compounds_by_taxon_query("Q122679");
    for relation in lotus_model::taxon_nomenclature::ALL {
        for (property, prefixed) in relation.properties() {
            assert!(
                query.contains(prefixed),
                "{property} ({}) is in the nomenclatural closure",
                relation.id
            );
        }
    }
    assert!(
        !query.contains("P1531"),
        "hybrid-of is not a rename and does not belong in the closure"
    );
}

/// Each toggle puts exactly its own two properties in the path, and only those.
///
/// This is the test that would fail if the four were collapsed back into one
/// "synonyms" switch: the relationships are independent, and a user who turns
/// off the basionym should not lose the replacement name with it.
#[test]
fn each_toggle_brings_only_its_own_properties() {
    for (nomenclature, relation) in [
        (
            Nomenclature::accepted_synonyms_only(),
            lotus_model::taxon_nomenclature::ACCEPTED_SYNONYM,
        ),
        (
            Nomenclature::basionyms_only(),
            lotus_model::taxon_nomenclature::BASIONYM,
        ),
        (
            Nomenclature::protonyms_only(),
            lotus_model::taxon_nomenclature::PROTONYM,
        ),
        (
            Nomenclature::replacements_only(),
            lotus_model::taxon_nomenclature::REPLACEMENT,
        ),
    ] {
        let query = compounds_by_taxon_query_with("Q122679", &nomenclature);
        assert_eq!(
            query.matches("?seed (").count(),
            1,
            "one expansion subquery per search, whatever is enabled"
        );
        assert_eq!(
            relation.properties().len(),
            2,
            "each relation is a pair of properties"
        );
        for (property, prefixed) in relation.properties() {
            assert!(query.contains(prefixed), "{property} is present");
        }
        for other in lotus_model::taxon_nomenclature::ALL {
            if other.id == relation.id {
                continue;
            }
            for (property, prefixed) in other.properties() {
                assert!(
                    !query.contains(prefixed),
                    "{property} belongs to {} and must not be here",
                    other.id
                );
            }
        }
    }
}

/// Every enabled property is read in both directions.
///
/// Wikidata stores each relationship from both ends, and a curator enters
/// whichever end they are looking at: `Rosmarinus officinalis` carries `P12764`
/// toward `Salvia rosmarinus` while `Salvia rosmarinus` carries `P694` back. A
/// one-way path would find the newer name from the older one and not the
/// reverse.
#[test]
fn the_closure_is_read_in_both_directions() {
    let query = compounds_by_taxon_query("Q122679");
    let path = closure_path(&query);
    for relation in lotus_model::taxon_nomenclature::ALL {
        for (property, prefixed) in relation.properties() {
            assert!(
                path.contains(&format!("{prefixed}|")) || path.ends_with(prefixed),
                "{property} is read forwards"
            );
            assert!(
                path.contains(&format!("^{prefixed}|")) || path.ends_with(&format!("^{prefixed}")),
                "{property} is also read backwards"
            );
        }
    }
}

/// The seed is expanded in its own subquery, and only the resulting handful of
/// QIDs is handed to `P171*`.
///
/// This is the efficiency decision the whole feature rests on, and the two
/// alternatives are both worse: interleaving the closures as
/// `(wdt:P171*|SYN*)` re-walks the nomenclatural closure at every node of the
/// tree, and expanding synonyms for each descendant would admit the other names
/// of taxa the user never asked about.
#[test]
fn the_closure_dedupes_its_properties() {
    let query = compounds_by_taxon_query("Q122679");
    let path = closure_path(&query);
    // Each predicate appears exactly twice: once forward, once under `^`. The
    // count is over the alternatives rather than the text, because `wdt:P1420`
    // is a substring of `^wdt:P1420` and the text would double-count.
    let alternatives: Vec<&str> = path.split('|').collect();
    let mut expected = 0;
    for relation in lotus_model::taxon_nomenclature::ALL {
        for (property, prefixed) in relation.properties() {
            assert_eq!(
                alternatives.iter().filter(|a| **a == prefixed).count(),
                1,
                "{property} is read forward exactly once"
            );
            assert_eq!(
                alternatives
                    .iter()
                    .filter(|a| **a == format!("^{prefixed}"))
                    .count(),
                1,
                "{property} is read backward exactly once"
            );
            expected += 2;
        }
    }
    assert_eq!(
        alternatives.len(),
        expected,
        "no predicate is written into the path twice"
    );
}

/// Turning every relationship off restores the query that was there before,
/// byte for byte. A user who wants the name as typed should not pay for the
/// names they did not ask about.
#[test]
fn turning_everything_off_restores_the_plain_ancestry_filter() {
    let off = compounds_by_taxon_query_with("Q122679", &Nomenclature::ALL_OFF);
    assert!(off.contains("?t (wdt:P171*) wd:Q122679 ."));
    assert!(!off.contains("VALUES ?seed"), "no seed to expand");
    for relation in lotus_model::taxon_nomenclature::ALL {
        for (property, prefixed) in relation.properties() {
            assert!(!off.contains(prefixed), "{property} is not reached");
        }
    }
    // Same nesting as the no-taxon query: SELECT plus two subqueries.
    assert_eq!(
        subquery_depth(&off),
        subquery_depth(&all_compounds_query()),
        "adding the options must not deepen the query when they are all off"
    );
}

/// On by default, all four of them. `compounds_by_taxon_query` is the form most
/// call sites use and it must not be the narrow one.
#[test]
fn every_relationship_is_included_unless_asked_otherwise() {
    let criteria = criteria();
    let names = criteria.taxon_names;
    assert!(names.accepted_synonyms, "accepted/synonym defaults on");
    assert!(names.basionyms, "basionym defaults on");
    assert!(names.protonyms, "original combination defaults on");
    assert!(names.replacements, "replacement name defaults on");
    assert!(criteria.has_nomenclatural_relations());

    assert_eq!(
        Nomenclature::default(),
        Nomenclature::ALL_ON,
        "the default is all four"
    );
    assert_eq!(
        compounds_by_taxon_query("Q122679"),
        compounds_by_taxon_query_with("Q122679", &Nomenclature::ALL_ON),
        "the default builder and the options agree"
    );
}

/// The four are independent: turning one off leaves the other three in the
/// query. Collapsing them into one switch would make this fail.
#[test]
fn a_structure_search_honours_the_same_relationships() {
    let on = structure_search_query_with(
        "CCO",
        SmilesSearchType::Substructure,
        0.8,
        Some("Q122679"),
        &Nomenclature::ALL_ON,
    );
    assert!(on.contains("VALUES ?seed { wd:Q122679 }"));
    assert!(on.contains("?t (wdt:P171*) ?root ."));

    let off = structure_search_query_with(
        "CCO",
        SmilesSearchType::Substructure,
        0.8,
        Some("Q122679"),
        &Nomenclature::ALL_OFF,
    );
    assert!(off.contains("?t (wdt:P171*) wd:Q122679 ."));
    assert!(!off.contains("VALUES ?seed"));
}

/// The wildcard is not a taxon and has no entity to expand, so it keeps the
/// root-anchored `P171*` and gains nothing from the closure.
#[test]
fn the_all_taxa_wildcard_has_nothing_to_expand() {
    let query = all_compounds_query();
    assert!(!query.contains("VALUES ?seed"));
    for relation in lotus_model::taxon_nomenclature::ALL {
        for (property, prefixed) in relation.properties() {
            assert!(!query.contains(prefixed), "{property} is absent");
        }
    }
}

/// The property path inside the expansion subquery, or a panic naming the test.
fn closure_path(query: &str) -> &str {
    query
        .split("?seed (")
        .nth(1)
        .and_then(|rest| rest.split(')').next())
        .expect("the closure is a property path")
}

/// `P12764` (*replaced synonym of*) and `P694` (*replaced synonym*, the
/// `nom. nov.` case) record the same relationship, and Wikidata uses both
/// inconsistently — some replacements are recorded only as `P12764`, with no
/// `P694` anywhere on the item. A search that read only `P694` would silently
/// miss those, so both travel together under one toggle.
#[test]
fn both_replaced_synonym_properties_travel_together() {
    let query = compounds_by_taxon_query_with("Q122679", &Nomenclature::replacements_only());
    assert!(query.contains("wdt:P12764"), "the general property");
    assert!(query.contains("wdt:P694"), "the nom. nov. case");
}

#[test]
fn no_taxon_means_no_ancestry_filter() {
    let query = all_compounds_query();
    assert!(!query.contains("P171*"));
    assert!(!query.contains("VALUES ?seed"), "nothing to expand");
    // One shallower than the taxon query, which spends a level on the
    // expansion. Every other level is the same.
    assert_eq!(
        subquery_depth(&query) + 1,
        subquery_depth(&compounds_by_taxon_query("Q16521"))
    );
}

/// A resolved closure is inlined, and an absent one falls back to the subquery.
///
/// The two halves are one behaviour: the fast form is only correct when it is
/// given something. An empty `VALUES` list is a valid SPARQL query that matches
/// nothing, so a closure resolution that returned nothing must degrade to the
/// slower correct form rather than to an empty answer.
///
/// Measured on `Q21754` (`docs/SPARQL-VARIANTS.md`): the inlined form returns
/// identical rows in 4,860 ms against 9,716 ms. What cannot be checked offline is
/// that the rows are identical, which is why the numbers there come from the
/// endpoint rather than from here.
#[test]
fn a_resolved_closure_is_inlined_and_an_empty_one_falls_back() {
    let nomenclature = lotus_query::Nomenclature::ALL_ON;
    let roots = ["Q2102991".to_string(), "Q21754".to_string()];

    let inlined = lotus_query::compounds_by_taxon_query_with_resolved_closure(
        "Q21754",
        &nomenclature,
        &roots,
    );

    assert!(
        inlined.contains("VALUES ?root { wd:Q2102991 wd:Q21754 }"),
        "the resolved closure is spliced in as a VALUES list:\n{inlined}"
    );
    assert!(
        !inlined.contains("SELECT DISTINCT ?root"),
        "and the subquery that would have computed it is gone:\n{inlined}"
    );
    assert!(
        inlined.contains("?t (wdt:P171*) ?root ."),
        "descent from the closure is still a P171* path:\n{inlined}"
    );

    // The guard: empty means "not resolved", not "matches nothing".
    let fell_back =
        lotus_query::compounds_by_taxon_query_with_resolved_closure("Q21754", &nomenclature, &[]);
    assert!(
        !fell_back.contains("VALUES ?root"),
        "an empty closure must not become an empty VALUES list:\n{fell_back}"
    );
    assert!(
        fell_back.contains("SELECT DISTINCT ?root"),
        "it must fall back to the subquery form, which is slower and correct:\n{fell_back}"
    );

    // And the default builder is untouched by any of this.
    let by_default = lotus_query::compounds_by_taxon_query_with("Q21754", &nomenclature);
    assert_eq!(
        by_default, fell_back,
        "the existing entry point must still produce the subquery form byte for byte"
    );
}

/// With every relationship off there is no closure to resolve, so the fast form
/// and the slow form are the same query.
#[test]
fn with_nomenclature_off_there_is_no_closure_to_inline() {
    let off = lotus_query::Nomenclature::ALL_OFF;
    let roots = ["Q2102991".to_string()];

    let with_roots =
        lotus_query::compounds_by_taxon_query_with_resolved_closure("Q21754", &off, &roots);
    let without_roots = lotus_query::compounds_by_taxon_query_with("Q21754", &off);

    assert_eq!(
        with_roots, without_roots,
        "nomenclature off means a bare P171* from the seed, and a resolved closure \
         must not smuggle one in"
    );
}
