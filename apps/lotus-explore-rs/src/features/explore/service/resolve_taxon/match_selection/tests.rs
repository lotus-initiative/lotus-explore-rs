// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `match_selection`, in their own file.

#![allow(clippy::expect_used)]

use super::*;
use crate::features::explore::types::LookupNotice;

fn candidate(name: &str, qid: &str) -> TaxonMatch {
    TaxonMatch {
        qid: qid.into(),
        name: name.into(),
        source: TaxonNameSource::Scientific,
    }
}

fn common(name: &str, qid: &str) -> TaxonMatch {
    TaxonMatch {
        qid: qid.into(),
        name: name.into(),
        source: TaxonNameSource::Common,
    }
}

#[test]
fn exact_match_is_preferred_over_first_non_exact_candidate() {
    let matches = vec![candidate("Rosa rubiginosa", "Q2"), candidate("Rosa", "Q1")];

    let selection = pick_best_match("rosa", &matches).expect("selection should succeed");

    assert_eq!(selection.best.qid, "Q1");
    assert_eq!(selection.candidates.len(), 0);
    assert_eq!(selection.to_cached().warnings().len(), 0);
}

#[test]
fn multiple_candidates_without_exact_match_emit_ambiguity_warning() {
    let matches = vec![
        candidate("Rosa rubiginosa", "Q2"),
        candidate("Rosa canina", "Q3"),
    ];

    let selection = pick_best_match("rosa", &matches).expect("selection should succeed");

    assert_eq!(selection.best.qid, "Q2");
    let cached = selection.to_cached();
    assert_eq!(
        cached.candidates,
        vec!["Rosa rubiginosa (Q2)", "Rosa canina (Q3)"]
    );
    assert_eq!(cached.warnings().len(), 1);
}

#[test]
fn duplicate_exact_matches_emit_ambiguity_warning() {
    let matches = vec![candidate("Rosa", "Q1"), candidate("rosa", "Q2")];

    let selection = pick_best_match("rosa", &matches).expect("selection should succeed");

    assert_eq!(selection.best.qid, "Q1");
    assert_eq!(selection.to_cached().warnings().len(), 1);
}

#[test]
fn the_candidate_list_is_truncated_but_never_empty() {
    let matches = (0..6)
        .map(|i| candidate(&format!("Taxon {i}"), &format!("Q{i}")))
        .collect::<Vec<_>>();

    let selection = pick_best_match("taxon", &matches).expect("selection should succeed");

    let cached = selection.to_cached();
    assert_eq!(cached.candidates.len(), MAX_LISTED_CANDIDATES);
    assert_eq!(cached.warnings().len(), 1);
}

// ── Scientific against common ────────────────────────────────────────────

#[test]
fn a_scientific_name_beats_a_common_name_at_equal_exactness() {
    // `Bacteria` is the scientific name of Q10876 and a common name of Q4034791;
    // ordered the other way round, every bacteria search goes to the wrong
    // kingdom.
    let matches = vec![
        common("Bacteria", "Q4034791"),
        candidate("Bacteria", "Q10876"),
    ];

    let selection = pick_best_match("Bacteria", &matches).expect("selection should succeed");

    assert_eq!(selection.best.qid, "Q10876");
    assert_eq!(selection.best.source, TaxonNameSource::Scientific);
    // Two exact readings, so still ambiguous — and the notice must not claim
    // the search fell back to a common name when it did not.
    assert_eq!(selection.candidates.len(), 2);
    assert_eq!(selection.to_cached().warnings().len(), 1);
}

#[test]
fn a_common_name_is_used_when_no_scientific_name_matches() {
    let matches = vec![common("Gentian", "Q777")];

    let selection = pick_best_match("Gentian", &matches).expect("selection should succeed");

    assert_eq!(selection.best.qid, "Q777");
    assert_eq!(selection.best.source, TaxonNameSource::Common);
    // Unambiguous, but the source still earns its own notice.
    assert_eq!(selection.candidates.len(), 0);
    assert_eq!(selection.to_cached().warnings().len(), 1);
}

#[test]
fn an_exact_scientific_name_beats_a_partial_scientific_one() {
    // Exactness outranks the source: still two scientific names, and the one
    // typed is the answer.
    let matches = vec![
        candidate("Gentiana", "Q1"),
        candidate("Gentiana lutea", "Q2"),
    ];

    let selection = pick_best_match("gentiana", &matches).expect("selection should succeed");

    assert_eq!(selection.best.qid, "Q1");
}

#[test]
fn exactness_outranks_the_source() {
    // The scientific name is a different organism entirely. Exactness decides
    // first, so the common name wins — and still says so, being one.
    let matches = vec![
        common("Gentiana", "Q777"),
        candidate("Gentiana lutea", "Q2"),
    ];

    let selection = pick_best_match("Gentiana", &matches).expect("selection should succeed");

    assert_eq!(selection.best.qid, "Q777");
    assert_eq!(selection.best.source, TaxonNameSource::Common);
    let warnings = selection.to_cached().warnings();
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings
            .iter()
            .all(|w| matches!(w, LookupNotice::CommonName { .. })),
        "the only notice is the one about the source: {warnings:?}"
    );
}

#[test]
fn a_common_name_that_matches_nothing_exactly_still_wins_over_nothing() {
    // Partial reading: the common name is the best available answer, and the
    // ambiguity notice lists the alternatives.
    let matches = vec![common("Gentian", "Q777"), candidate("Gentiana lutea", "Q2")];

    let selection = pick_best_match("Gentia", &matches).expect("selection should succeed");

    assert_eq!(selection.best.qid, "Q777");
    assert_eq!(selection.candidates.len(), 2);
}
