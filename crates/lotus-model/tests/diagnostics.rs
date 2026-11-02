// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! The memory the set holds, as the diagnostics feature reports it.
//!
//! Behind `diagnostics`, so 103 lines of it were uncovered: the feature is off by
//! default and no test in this crate enabled it. That matters more than the line
//! count suggests -- the number is the justification for the whole columnar
//! design, and there are two functions computing it independently. Nothing said
//! they agreed, and nothing would notice if a column were added to one and not
//! the other: the reported figure would simply be wrong, and the figure is what
//! a reader checks the design against.

// The panic lints keep library code free of panics on external input. A test
// failing on a bad fixture is reporting, not panicking.
#![allow(unused_crate_dependencies)]
#![allow(clippy::expect_used, clippy::indexing_slicing)]
#![cfg(feature = "diagnostics")]

use lotus_model::testing::full_row;
use lotus_model::{ColumnarBuilder, CompoundEntry, RawRow};
use std::sync::Arc;

fn arc(text: &str) -> Arc<str> {
    Arc::from(text)
}

/// A set built from `rows`, through the builder so every dictionary is filled.
fn set_of(rows: &[CompoundEntry]) -> lotus_model::ColumnarResultSet {
    let mut builder = ColumnarBuilder::new();
    for row in rows {
        builder.push(RawRow {
            compound_qid: &row.compound_qid,
            name: &row.name,
            inchikey: row.inchikey.as_deref(),
            smiles: row.smiles.as_deref(),
            mass: row.mass,
            formula: row.formula.as_deref(),
            taxon_qid: &row.taxon_qid,
            taxon_name: &row.taxon_name,
            reference_qid: &row.reference_qid,
            reference_node: &row.reference_node,
            ref_title: row.ref_title.as_deref(),
            ref_doi: row.ref_doi.as_deref(),
            pub_year: row.pub_year,
            statement: row.statement.as_deref(),
        });
    }
    builder.build()
}

#[test]
fn an_empty_set_reports_nothing_rather_than_a_nonzero_number() {
    let set = lotus_model::ColumnarResultSet::default();
    assert_eq!(
        set.total_dictionary_bytes(),
        0,
        "an empty set holds nothing, so the figure must be zero rather than a \\
         few bytes of per-dictionary overhead"
    );
    assert!(
        set.dictionary_costs()
            .iter()
            .all(|(_, count, _)| *count == 0),
        "and every dictionary must report zero entries: {:?}",
        set.dictionary_costs()
    );
}

#[test]
fn a_longer_value_costs_more_and_a_second_row_costs_almost_nothing() {
    // The two properties that make the number mean anything. If a longer label
    // did not cost more, the figure was measuring nothing; and if a second row
    // cost proportionally, the dictionaries were not deduplicating, which is the
    // entire reason the type exists.
    let short = set_of(&[CompoundEntry {
        name: arc("a"),
        ..full_row()
    }]);
    let long = set_of(&[CompoundEntry {
        name: arc(&"a".repeat(1_000)),
        ..full_row()
    }]);
    assert!(
        long.total_dictionary_bytes() > short.total_dictionary_bytes(),
        "a thousand more characters of label must cost more"
    );

    let one = set_of(&[full_row()]);
    let two = set_of(&[full_row(), full_row()]);
    assert!(
        two.total_dictionary_bytes() <= one.total_dictionary_bytes() * 2,
        "a second row sharing every value must not double the dictionaries: \\
         {} against {}",
        two.total_dictionary_bytes(),
        one.total_dictionary_bytes()
    );
}

#[test]
fn every_dictionary_is_named_and_counted() {
    let set = set_of(&[full_row()]);
    let costs = set.dictionary_costs();

    // Named, so a dictionary added to the set and not to this list would be
    // visible here rather than only in the total.
    for expected in [
        "compound qids",
        "taxon qids",
        "reference qids",
        "compound names",
        "compound inchikeys",
        "statement fallbacks",
    ] {
        assert!(
            costs.iter().any(|(name, _, _)| *name == expected),
            "no dictionary reported as {expected:?}: {:?}",
            costs.iter().map(|(n, _, _)| *n).collect::<Vec<_>>()
        );
    }
    assert!(
        costs.iter().all(|(_, _, bytes)| *bytes > 0),
        "a populated set must cost something for every dictionary: {costs:?}"
    );
}

/// `statement_fallbacks` is an id -> string interning table, not a slot-keyed
/// column, so it never fills the slot column that `len()` counts. Its entry count
/// used to be read from there and was always zero while its bytes were not; it
/// now comes from `distinct_len()`, the same figure as `statement_fallback_entries`.
#[test]
fn the_statement_fallback_dictionary_counts_its_entries() {
    let set = set_of(&[full_row()]);
    let fallback = set
        .dictionary_costs()
        .into_iter()
        .find(|(name, _, _)| *name == "statement fallbacks")
        .expect("the dictionary is reported");

    assert_eq!(fallback.1, 1);
    assert_eq!(fallback.1, set.statement_fallback_entries());
    assert!(fallback.2 > 0, "{fallback:?}");
}
