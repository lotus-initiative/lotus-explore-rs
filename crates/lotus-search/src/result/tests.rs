// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `result`, in their own file.

use super::{SearchResult, TaxonNote, TaxonResolution};
use lotus_model::CompoundEntry;

fn resolution(qid: Option<&str>, looked_up: &str) -> TaxonResolution {
    TaxonResolution {
        qid: qid.map(str::to_string),
        looked_up: looked_up.to_string(),
        notes: Vec::new(),
    }
}

/// A resolved taxon reports the name the lookup used, and no name for an
/// empty lookup.
///
/// `looked_up_name` is what the UI shows next to the results, so its two
/// failure modes are different bugs: reporting a name for a resolution that
/// has none puts an empty string on screen, and refusing to report a name
/// that exists leaves the reader with a bare QID and no idea what was searched.
#[test]
fn a_resolution_reports_the_name_it_looked_up() {
    assert_eq!(
        resolution(Some("Q16521"), "Gentiana lutea").looked_up_name(),
        Some("Gentiana lutea")
    );
    assert_eq!(
        resolution(None, "Gentianales").looked_up_name(),
        Some("Gentianales"),
        "a wildcard has no QID but still has the text the reader typed"
    );
    assert_eq!(
        resolution(None, "").looked_up_name(),
        None,
        "an empty lookup has no name to report, and must not report an empty one"
    );
    assert_eq!(
        resolution(Some("Q16521"), "").looked_up_name(),
        None,
        "a QID input with no looked-up text reports no name"
    );
}

/// Every taxon is the absence of a QID, and nothing else.
///
/// This is what decides whether the result set is described as unfiltered, so
/// a `*` search and a taxon search must not agree. Both directions matter: the
/// constant `true` would label every search as covering all taxa, and the
/// constant `false` would label none of them.
#[test]
fn every_taxon_is_exactly_the_absence_of_a_qid() {
    assert!(
        resolution(None, "*").is_all_taxa(),
        "a wildcard covers every taxon"
    );
    assert!(
        resolution(None, "").is_all_taxa(),
        "an empty input constrains no taxon"
    );
    assert!(
        !resolution(Some("Q16521"), "Gentiana lutea").is_all_taxa(),
        "a named taxon is one taxon, not every taxon"
    );
}

/// The rows come back as the rows that are stored, in order, and nothing else.
///
/// `as_rows` is the hand-off from a page of rows to a table or an export, so
/// a default-constructed `Rows` here would be an empty export that reported
/// success.
#[test]
fn the_rows_are_the_rows_that_were_stored() {
    let mut result = SearchResult::default();
    assert!(
        result.as_rows().is_empty(),
        "a result with no rows has no rows"
    );

    result.rows = (1..=3)
        .map(|n| CompoundEntry {
            compound_qid: format!("Q{n}").into(),
            name: format!("compound-{n}").into(),
            ..CompoundEntry::default()
        })
        .collect();

    let rows = result.as_rows();
    assert_eq!(rows.len(), 3, "every stored row is handed over");
    let handed_over: Vec<&str> = rows.iter().map(|r| r.compound_qid.as_ref()).collect();
    assert_eq!(
        handed_over,
        vec!["Q1", "Q2", "Q3"],
        "and in the order they were stored"
    );
}

/// Both notes say what they are, in words a reader can act on.
///
/// These strings go straight into the UI as the explanation for a search that
/// did something other than what was typed, so "the first of N" has to carry
/// the count and the standardisation has to carry both spellings. A `Display`
/// that returned nothing would render as an empty note rather than as a
/// missing one.
#[test]
fn a_note_explains_itself() {
    let standardized = TaxonNote::Standardized {
        original: "gentiana lutea".to_string(),
        looked_up: "Gentiana lutea".to_string(),
    }
    .to_string();
    assert!(
        standardized.contains("gentiana lutea") && standardized.contains("Gentiana lutea"),
        "a standardisation has to name both what was typed and what was used: \
         {standardized}"
    );

    let ambiguous = TaxonNote::Ambiguous {
        candidates: vec![
            "Gentiana lutea (Q16521)".to_string(),
            "Gentiana (Q21754)".to_string(),
        ],
    }
    .to_string();
    assert!(
        ambiguous.contains('2'),
        "an ambiguity has to say how many matched: {ambiguous}"
    );
    assert!(
        ambiguous.contains("Gentiana lutea (Q16521)"),
        "and name them: {ambiguous}"
    );
}
