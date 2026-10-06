// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Two rules in the query builder that decide what is actually asked for.
//!
//! Neither had a test, and neither is visible from the outside: one rewrites
//! the taxon, and one decides which builder runs. Both were uncovered branches
//! in `build_execution_query_with`.

// The panic lints keep library code free of panics on external input. A test
// failing on a bad fixture is reporting, not panicking.
#![allow(unused_crate_dependencies)]
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use lotus_model::{SearchCriteria, SmilesSearchType};
use lotus_search::{
    ResolvedInputs, SearchRequest, build_execution_query_with, normalize_structure,
};

const fn request(criteria: SearchCriteria) -> SearchRequest {
    SearchRequest::new(criteria, 2026)
}

/// The criteria every test here starts from: no taxon, no structure, no
/// reference, every matching row.
const fn bare() -> SearchCriteria {
    SearchCriteria::up_to_year(2026)
}

/// The QID a `*` taxon is rewritten to when the query is by compound.
const WILDCARD_AS_QID: &str = "Q2382443";

#[test]
fn an_exact_compound_search_by_wildcard_taxon_asks_for_a_named_taxon() {
    // The compound QIDs come from a structure lookup that resolved them, and the
    // taxon is `*` -- every taxon with a recorded occurrence. Passed through as
    // written, the endpoint has no taxon to constrain and the result set is
    // everything, which is not what "this compound, in every taxon" means: the
    // query has to name the taxon the wildcard stands for.
    let mut criteria = bare();
    criteria.structure_search = SmilesSearchType::Exact;
    let resolved = ResolvedInputs {
        compounds: vec!["Q3613679".to_string()],
        reference_qid: None,
    };

    let wildcard = build_execution_query_with(&request(criteria.clone()), Some("*"), &resolved);
    let named =
        build_execution_query_with(&request(criteria.clone()), Some(WILDCARD_AS_QID), &resolved);
    assert_eq!(
        wildcard, named,
        "a wildcard taxon on an exact compound search must ask for the same \
         query as the taxon the wildcard stands for"
    );
    assert!(
        wildcard.contains(WILDCARD_AS_QID),
        "and that query must name it: {wildcard}"
    );
    // Not "no asterisk": `P171*` is a legitimate property path and the query
    // has one. What must not happen is the asterisk being used as a taxon value.
    assert!(
        !wildcard.contains("wd:*") && !wildcard.contains("VALUES ?taxon { * }"),
        "the wildcard must not reach the endpoint as a taxon: {wildcard}"
    );

    // And a named taxon is passed through untouched -- the rewrite is for the
    // wildcard, not for every taxon.
    let other = build_execution_query_with(&request(criteria), Some("Q16521"), &resolved);
    assert!(
        other.contains("Q16521") && !other.contains(WILDCARD_AS_QID),
        "a named taxon must be used as written: {other}"
    );
}

#[test]
fn a_broad_structure_search_keeps_the_structure_and_ignores_the_resolved_compounds() {
    // The resolved QIDs are what an *exact* search goes by. A substructure search
    // is about the compounds containing the structure, which is a different
    // question, so it must keep asking the structure service rather than
    // collapsing onto the one compound the name happened to resolve to.
    let resolved = ResolvedInputs {
        compounds: vec!["Q3613679".to_string()],
        reference_qid: None,
    };

    for mode in [SmilesSearchType::Substructure, SmilesSearchType::Similarity] {
        let mut criteria = bare();
        criteria.structure = "CCO".to_string();
        criteria.structure_search = mode;
        let query = build_execution_query_with(&request(criteria), None, &resolved);

        assert!(
            !query.contains("Q3613679"),
            "{mode:?} must not collapse onto the resolved compound: {query}"
        );
        assert!(
            query.contains("CCO"),
            "{mode:?} must still send the structure: {query}"
        );
    }
}

#[test]
fn a_resolved_reference_replaces_the_doi_the_user_typed() {
    // The DOI is what was typed; the QID is what Wikidata calls it. The query
    // goes by QID, because a DOI string in an equality test is a string
    // comparison against a differently-normalised value.
    let mut criteria = bare();
    criteria.reference = "10.1000/xyz".to_string();
    let resolved = ResolvedInputs {
        compounds: Vec::new(),
        reference_qid: Some("Q100000001".to_string()),
    };

    let query = build_execution_query_with(&request(criteria.clone()), None, &resolved);
    assert!(
        query.contains("Q100000001"),
        "the QID must be used: {query}"
    );
    assert!(
        !query.contains("10.1000/xyz"),
        "the DOI the user typed must not also be in the query: {query}"
    );

    // With nothing resolved, the typed value stands -- the reference lookup did
    // not answer, and dropping the filter entirely would widen the search
    // without saying so.
    let unresolved = ResolvedInputs {
        compounds: Vec::new(),
        reference_qid: None,
    };
    let query = build_execution_query_with(&request(criteria), None, &unresolved);
    assert!(
        query.contains("10.1000/xyz"),
        "an unresolved reference must keep the user's own value: {query}"
    );
}

#[test]
fn a_molfile_keeps_its_padding_while_anything_else_is_trimmed() {
    // Line endings are unified for both. After that the two shapes are treated
    // differently, and the difference is the point:
    //
    // - A molfile's leading blank lines and counts line are the format's own
    //   padding, not the user's. Trimming them changes the file.
    // - Anything else is a SMILES, where surrounding whitespace is not part of
    //   the molecule and trimming is tidying, not a guess.
    //
    // So `\n\n\n  1  0  0  0  0  0  0  0  0  0999 V2000\n...M  END\n` survives
    // whole, and `"  CCO  "` does not.
    let crlf = "line one\r\nline two\r\n";
    assert_eq!(
        normalize_structure(crlf),
        "line one\nline two",
        "CRLF becomes LF, and the trailing blank is trimmed away with it"
    );
    assert_eq!(
        normalize_structure("bare\rreturn"),
        "bare\nreturn",
        "a bare CR is a line ending too"
    );

    // A molfile is recognised by its version tag and `M  END`, and is passed
    // through whole.
    let molfile = "\r\n\r\n\r\n  1  0  0  0  0  0  0  0  0  0999 V2000\r\nM  END\r\n";
    assert_eq!(
        normalize_structure(molfile),
        "\n\n\n  1  0  0  0  0  0  0  0  0  0999 V2000\nM  END\n",
        "a molfile keeps its padding: the counts line is at line 4 because the \
         format says so, and trimming it moves every atom"
    );

    // A SMILES is trimmed, and an interior space is left alone because it is
    // part of the string the user pasted.
    assert_eq!(normalize_structure("  CCO  "), "CCO");
    assert_eq!(
        normalize_structure("C C O"),
        "C C O",
        "an interior space is the user's, not padding"
    );
}
