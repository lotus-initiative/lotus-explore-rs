// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `structure_cache`, in their own file.

#![allow(clippy::expect_used, clippy::panic)]

use super::*;

fn match_on(qid: &str, label: &str) -> CompoundMatch {
    match_with(qid, label, "")
}

fn match_with(qid: &str, label: &str, smiles: &str) -> CompoundMatch {
    CompoundMatch {
        qid: qid.into(),
        label: label.into(),
        canonical_smiles: smiles.into(),
    }
}

#[test]
fn an_unmatched_input_is_remembered_as_unmatched() {
    clear();
    store("cc", None);
    assert_eq!(
        lookup("CC"),
        Cached::Unresolved,
        "a second search must not pay for the same miss"
    );
}

#[test]
fn an_input_never_looked_up_is_distinguishable_from_a_miss() {
    // The distinction the whole cache exists for: only the second is worth
    // asking about again.
    clear();
    store("cc", None);
    assert_eq!(lookup("CCC"), Cached::Unknown);
}

#[test]
fn a_blank_key_is_never_cached() {
    clear();
    store(
        "",
        Some(CachedCompound {
            qid: "Q1".into(),
            label: "x".into(),
            canonical_smiles: "C".into(),
            candidates: Vec::new(),
            all_qids: Vec::new(),
        }),
    );
    assert_eq!(lookup(""), Cached::Unknown);
}

#[test]
fn a_resolution_round_trips_case_insensitively() {
    clear();
    let cached = CachedCompound {
        qid: "Q18216".into(),
        label: "aspirin".into(),
        canonical_smiles: "CC(=O)OC1=CC=CC=C1C(=O)O".into(),
        candidates: Vec::new(),
        all_qids: Vec::new(),
    };
    store("Aspirin", Some(cached.clone()));
    assert_eq!(lookup("aspirin"), Cached::Resolved(Box::new(cached)));
}

#[test]
fn a_resolution_with_a_structure_is_kept_for_the_structure_service() {
    // Substructure and similarity searches hand this to the service, so the
    // compound's own SMILES has to survive the cache.
    clear();
    let cached = pick(&[match_with("Q18216", "aspirin", "CC(=O)O")]).expect("one match");
    assert_eq!(cached.canonical_smiles, "CC(=O)O");
    store("aspirin", Some(cached));
    let Cached::Resolved(remembered) = lookup("Aspirin") else {
        panic!("a resolution must be remembered");
    };
    assert_eq!(remembered.canonical_smiles, "CC(=O)O");
}

#[test]
fn among_several_structures_one_compound_carries_the_one_that_is_searched() {
    // A compound with several P233 values comes back as several rows. The
    // first is not necessarily the one with a structure.
    let cached = pick(&[
        match_with("Q1", "Aspirin", ""),
        match_with("Q1", "Aspirin", "CC(=O)O"),
    ])
    .expect("one compound");
    assert_eq!(cached.canonical_smiles, "CC(=O)O");
    assert!(cached.candidates.is_empty(), "still one compound");
}

#[test]
fn a_compound_with_no_label_is_still_cached() {
    // The parse keeps the row even with nothing to show for it. Dropping the
    // match here would make the miss unobservable and so un-cacheable.
    clear();
    let cached = pick(&[match_on("Q1", "")]).expect("one match");
    store("nameless", Some(cached.clone()));
    assert_eq!(lookup("nameless"), Cached::Resolved(Box::new(cached)));
}

#[test]
fn an_unambiguous_match_earns_only_the_resolved_notice() {
    let cached = pick(&[match_on("Q18216", "aspirin")]).expect("one match");
    assert_eq!(cached.notices().len(), 1);
    assert_eq!(cached.candidates.len(), 0);
}

#[test]
fn several_matches_earn_both_notices_and_list_the_candidates() {
    let cached =
        pick(&[match_on("Q1", "Aspirin"), match_on("Q2", "Aspirin B")]).expect("two matches");
    assert_eq!(cached.qid, "Q1", "the first match is the one used");
    assert_eq!(cached.candidates, vec!["Aspirin (Q1)", "Aspirin B (Q2)"]);
    assert_eq!(cached.notices().len(), 2);
}

#[test]
fn a_label_less_compound_is_described_by_its_qid() {
    let cached = pick(&[match_on("Q7", ""), match_on("Q8", "Named")]).expect("two matches");
    assert_eq!(cached.candidates, vec!["Q7", "Named (Q8)"]);
}

#[test]
fn no_matches_is_no_resolution() {
    assert_eq!(pick(&[]), None);
}
