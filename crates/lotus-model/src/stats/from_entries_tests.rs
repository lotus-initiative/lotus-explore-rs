// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `stats`, in their own file.

use super::*;

fn entry(compound: &str, taxon: &str, reference: &str) -> CompoundEntry {
    CompoundEntry {
        compound_qid: compound.into(),
        taxon_qid: taxon.into(),
        reference_qid: reference.into(),
        ..CompoundEntry::default()
    }
}

#[test]
fn counts_distinct_values_not_rows() {
    let rows = vec![
        entry("Q1", "Q10", "Q100"),
        entry("Q1", "Q10", "Q100"),
        entry("Q1", "Q11", "Q101"),
    ];
    let stats = DatasetStats::from_entries(&rows);
    assert_eq!(stats.n_compounds, 1, "the same compound three times");
    assert_eq!(stats.n_taxa, 2);
    assert_eq!(stats.n_references, 2);
    assert_eq!(stats.n_entries, 3, "rows are counted, not deduplicated");
    assert_eq!(stats.n_entries_unique, 2, "the repeated triple is one");
}

#[test]
fn an_empty_row_contributes_no_taxon_or_reference() {
    // A compound that matched with no occurrence and no citation is still a
    // compound; counting it as a taxon or a reference would report a
    // diversity the data does not have.
    let rows = vec![entry("Q1", "", "")];
    let stats = DatasetStats::from_entries(&rows);
    assert_eq!(stats.n_compounds, 1);
    assert_eq!(stats.n_taxa, 0);
    assert_eq!(stats.n_references, 0);
    assert_eq!(stats.n_entries_unique, 1);
}

#[test]
fn an_empty_slice_is_all_zero() {
    let stats = DatasetStats::from_entries(&[]);
    assert_eq!(stats.n_compounds, 0);
    assert_eq!(stats.n_entries, 0);
    assert_eq!(stats.n_entries_unique, 0);
}

#[test]
fn distinct_triples_can_exceed_distinct_values() {
    // The same two compounds, two taxa and two references give four triples
    // from four values; the two counts are not interchangeable, and an
    // implementation that computed one from the other would be wrong here.
    let rows = vec![
        entry("Q1", "Q10", "Q100"),
        entry("Q1", "Q11", "Q101"),
        entry("Q2", "Q10", "Q100"),
        entry("Q2", "Q11", "Q101"),
    ];
    let stats = DatasetStats::from_entries(&rows);
    assert_eq!(stats.n_compounds, 2);
    assert_eq!(stats.n_taxa, 2);
    assert_eq!(stats.n_references, 2);
    assert_eq!(stats.n_entries_unique, 4);
}
