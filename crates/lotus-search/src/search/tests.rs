// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `search`, in their own file.

use super::{build_base_query, is_ambiguous};
use lotus_model::{SearchCriteria, SmilesSearchType, TaxonMatch, TaxonNameSource};

/// More than one candidate is ambiguous; exactly one is not.
///
/// This decides whether the reader is told that the first of several matches was
/// used. Getting it wrong in the tight direction hides a real ambiguity, and in
/// the loose direction warns about a search that had no choice to make.
#[test]
fn only_several_candidates_are_ambiguous() {
    let candidate = |qid: &str| TaxonMatch {
        qid: qid.to_string(),
        name: format!("name-{qid}"),
        source: TaxonNameSource::Scientific,
    };
    let candidates = |n: usize| {
        (0..n)
            .map(|i| candidate(&format!("Q{}", 100 + i)))
            .collect::<Vec<_>>()
    };

    assert!(!is_ambiguous(&candidates(0)), "no match is not ambiguous");
    assert!(
        !is_ambiguous(&candidates(1)),
        "one match is a match, not an ambiguity"
    );
    assert!(is_ambiguous(&candidates(2)), "two matches are ambiguous");
    assert!(is_ambiguous(&candidates(5)), "and so are five");
}

/// A wildcard passed as a QID still builds the query that requires occurrences.
///
/// `build_base_query` reads the wildcard out of `criteria.taxon` now, because
/// `resolve_taxon` resolves `*` to no QID at all. This test covers the other
/// door: a caller that hands the wildcard straight through as a QID, which is
/// what the guard on that arm is for.
#[test]
fn a_wildcard_handed_straight_in_is_not_treated_as_a_qid() {
    let criteria = SearchCriteria {
        taxon: "*".to_string(),
        ..SearchCriteria::up_to_year(2026)
    };
    let via_qid = build_base_query(&criteria, Some("*"));

    assert!(
        !via_qid.contains("wd:*"),
        "the wildcard is not an entity and must never be spliced into a wd: IRI"
    );
    assert!(
        via_qid.contains("p:P703"),
        "a wildcard keeps requiring an occurrence however it arrived"
    );
}

/// The guard on the QID arm, reached the only way it can be: a wildcard QID with
/// a taxon box that does not itself say `*`.
///
/// The test above sets the box to `*` too, so the arm before this one answers it
/// and the guard is never asked. Here nothing else can: without the guard, `*`
/// would be spliced into `wd:*` as if it named an entity.
#[test]
fn a_wildcard_qid_under_any_other_taxon_still_means_every_taxon() {
    let criteria = SearchCriteria {
        taxon: "Gentiana".to_string(),
        ..SearchCriteria::up_to_year(2026)
    };
    assert_eq!(
        build_base_query(&criteria, Some("*")),
        lotus_query::all_compounds_query()
    );
}

/// A reference names occurrences, so a reference-only search requires one.
/// With occurrences optional, the filter's `VALUES ?r` joined every compound
/// that has none and the search returned the whole graph.
#[test]
fn a_reference_alone_requires_an_occurrence() {
    let criteria = SearchCriteria {
        reference: "Q108766178".to_string(),
        ..SearchCriteria::up_to_year(2026)
    };
    assert_eq!(
        build_base_query(&criteria, None),
        lotus_query::all_compounds_query()
    );
}

/// An empty box and `*` are different requests, and the difference has to
/// survive into the query text.
///
/// This is the assertion that keeps them apart. `resolve_taxon` maps both to
/// `None`, because neither names a Wikidata entity, so a query builder that
/// reads the taxon off the QID alone cannot tell them apart -- and did not,
/// which is why an empty box silently answered the narrower question. The
/// matching risk after the fix is the opposite one: someone tidying the
/// match arms until the two branches collapse back together. Nothing else
/// fails when that happens; the query just quietly stops including
/// untaxonomised compounds and the row count drops.
#[test]
fn an_empty_box_and_a_wildcard_are_not_the_same_request() {
    let blank = build_base_query(&SearchCriteria::up_to_year(2026), None);
    let wildcard = build_base_query(
        &SearchCriteria {
            taxon: "*".to_string(),
            ..SearchCriteria::up_to_year(2026)
        },
        None,
    );

    assert_ne!(
        blank, wildcard,
        "an empty box and `*` build the same query, so one of the two is a lie"
    );
    // The distinction is not the presence of the occurrence join -- both arms
    // have one, or the result would carry no rows at all -- it is whether the
    // join is required. `*` demands an occurrence; the empty box only takes one
    // if the compound happens to have it, which is the whole difference between
    // "every compound with an organism" and "every compound".
    let optional = |q: &str| {
        q.find("p:P703").is_some_and(|p703| {
            q[..p703]
                .rfind("OPTIONAL")
                .is_some_and(|at| !q[at..p703].contains('}'))
        })
    };
    assert!(
        !optional(&wildcard),
        "`*` is the explicit request for everything with an occurrence, so the \
         occurrence join is required rather than optional"
    );
    assert!(
        optional(&blank),
        "an empty box constrains no taxon, which includes having no occurrence to \
         name -- a required join would quietly exclude untaxonomised compounds"
    );
}

#[test]
fn a_structure_search_with_wildcard_qid_requires_occurrence_without_biota_closure() {
    let criteria = SearchCriteria {
        taxon: "*".to_string(),
        structure: "c1ccc2[nH]ccc2c1".to_string(),
        structure_search: SmilesSearchType::Substructure,
        ..SearchCriteria::up_to_year(2026)
    };
    let q = build_base_query(&criteria, Some("*"));
    assert!(
        q.contains("sachem:substructureSearch"),
        "expected substructure search:\n{q}"
    );
    assert!(
        q.contains("?c p:P703 ?statement_id .") && !q.contains("OPTIONAL {\n    ?c p:P703"),
        "wildcard QID substructure search must require an occurrence:\n{q}"
    );
    assert!(
        !q.contains("Q2382443") && !q.contains("P171*"),
        "wildcard QID must not rewrite to Biota P171* closure:\n{q}"
    );
}
