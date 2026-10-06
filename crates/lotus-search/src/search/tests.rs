// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `search`, in their own file.

use super::{build_base_query, is_ambiguous, is_qid, standardize_taxon_name};
use lotus_model::{SearchCriteria, TaxonMatch, TaxonNameSource};

/// Underscores become spaces and the genus is capitalised, because taxon names
/// arrive from spreadsheets in both shapes.
///
/// Both halves matter independently: an underscored name finds nothing as typed,
/// and a lower-case genus is a different string from the one Wikidata stores.
/// Mutation testing could replace this function with an empty string or with a
/// constant and the suite noticed neither, because no test fed it anything.
#[test]
fn a_taxon_name_is_standardised_before_it_is_looked_up() {
    assert_eq!(standardize_taxon_name("gentiana_lutea"), "Gentiana lutea");
    assert_eq!(standardize_taxon_name("Gentiana_lutea"), "Gentiana lutea");
    assert_eq!(standardize_taxon_name("gentiana lutea"), "Gentiana lutea");

    // Only the genus is touched: a species epithet keeps the case it was given,
    // because Wikidata stores `lutea` in lower case but `GENTIANA` is not a word.
    assert_eq!(
        standardize_taxon_name("gentiana LUTEA"),
        "Gentiana LUTEA",
        "only the genus is recased"
    );

    // A single word is still a genus.
    assert_eq!(standardize_taxon_name("gentianella"), "Gentianella");

    // Nothing in, nothing out. Input with no word in it is handed back
    // unchanged rather than trimmed to nothing: `resolve_taxon` trims the input
    // *before* calling this, so a blank box never reaches here, and a
    // standardiser that silently rewrote blank input would be inventing a name
    // the reader never typed.
    assert_eq!(standardize_taxon_name(""), "");
    assert_eq!(standardize_taxon_name("   "), "   ");
    assert_eq!(
        standardize_taxon_name("_"),
        " ",
        "an underscore becomes a space, and a lone underscore has no genus to recase"
    );
}

/// A QID is a `Q` and at least one digit, and nothing looser.
///
/// The digit requirement is the interesting half: `Q` alone is a search term,
/// and treating it as an identifier would send `wd:Q` to an endpoint that
/// cannot answer it.
#[test]
fn a_bare_qid_is_recognised_and_nothing_else_is() {
    assert!(is_qid("Q16521"));
    assert!(is_qid("q16521"), "a lower-case q is still a QID");

    assert!(!is_qid("Q"), "a lone Q is a search term, not an identifier");
    assert!(!is_qid("q"), "nor in lower case");
    assert!(!is_qid(""), "nor is an empty string");
    assert!(!is_qid("Q16521a"), "nor a QID with a suffix");
    assert!(!is_qid("P16521"), "a property is not an entity");
    assert!(!is_qid("Q-16521"), "nor a QID with a sign");
    assert!(!is_qid("Gentiana lutea"), "nor a name");
}

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
    let flat = |q: &str| q.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        !flat(&wildcard).contains("OPTIONAL { ?c p:P703"),
        "`*` is the explicit request for everything with an occurrence, so the \
         occurrence join is required rather than optional"
    );
    assert!(
        flat(&blank).contains("OPTIONAL { ?c p:P703"),
        "an empty box constrains no taxon, which includes having no occurrence to \
         name -- a required join would quietly exclude untaxonomised compounds"
    );
}
