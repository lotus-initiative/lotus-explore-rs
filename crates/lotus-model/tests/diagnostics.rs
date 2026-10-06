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
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
#![cfg(feature = "diagnostics")]

use lotus_model::{ColumnarBuilder, CompoundEntry, RawRow};
use std::sync::Arc;

fn arc(text: &str) -> Arc<str> {
    Arc::from(text)
}

/// One row with every optional column populated, so no dictionary is empty.
fn full_row() -> CompoundEntry {
    CompoundEntry {
        compound_qid: arc("Q3613679"),
        name: arc("quercetin"),
        inchikey: Some(arc("ABCDEF-GHIJKL-M")),
        smiles: Some(arc("O=c1cc(-c2ccccc2)oc2cc(O)cc(O)c12")),
        mass: Some(302.24),
        formula: Some(arc("C15H10O7")),
        taxon_qid: arc("Q128267"),
        taxon_name: arc("Rosa"),
        reference_qid: arc("Q100000001"),
        reference_node: arc("6eff7d028afee42232e3963a2c0f9d3b7e5a41c8d2f60b9e3a7d5c1f8b2e6049a"),
        ref_title: Some(arc("Flavonoid isolation, 1971")),
        ref_doi: Some(arc("10.1000/a, b")),
        pub_year: Some(1971),
        statement: Some(arc("Q200000002")),
    }
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
fn the_two_functions_agree_about_what_the_set_holds() {
    // The property worth having. `dictionary_costs` itemises and
    // `total_dictionary_bytes` sums, and they are written separately over the
    // same fields. A column added to one and not the other is a silent wrong
    // number, and nothing in the build fails for it.
    let set = set_of(&[full_row(), full_row()]);

    let costs: usize = set
        .dictionary_costs()
        .iter()
        .map(|(_, _, bytes)| bytes)
        .sum();
    let total = set.total_dictionary_bytes();

    assert!(
        total > 0,
        "a populated set holds something, so the figure is not zero"
    );
    // Direction-agnostic: measured, the two differ by about a tenth, in the
    // itemised figure's disfavour, because the QID estimate charges map slack the
    // total omits. Asserting closeness in one direction would have pinned the
    // wrong sign.
    let ratio = total
        .max(costs)
        .checked_div(total.min(costs).max(1))
        .unwrap_or(0);
    assert!(
        ratio <= 2,
        // Not asserted as equality: the two use different per-dictionary
        // arithmetic (`qids` counts map slack, the total does not), so they are
        // two estimates of one quantity and not two spellings of it. What must
        // hold is that they are the same order of magnitude -- a column added to
        // one function and not the other shows up here as a factor.
        "itemised {costs} against a total of {total}: a column added to one \\
         function and not the other would show as this gap"
    );
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

/// **Documents a defect. Read this before "fixing" the assertion.**
///
/// `statement_fallbacks` is a plain id -> string interning table, not a
/// slot-keyed column: `SparseStrings::intern` pushes to `values` and `index` and
/// never touches `ids`. So `len()` -- which is `ids.len()` -- is permanently
/// zero for it, and `dictionary_costs` reports that zero as its entry count:
///
/// ```text
/// ("statement fallbacks", self.statement_fallbacks.len(), sparse(&self.statement_fallbacks))
/// ```
///
/// The cost term is right and the count is not, because `sparse` counts `values`
/// while the count reads `ids`. So a set with one fallback reports
/// `("statement fallbacks", 0, 26)` -- zero entries, 26 bytes. `distinct_len()`
/// is the accessor that would answer correctly.
///
/// The same mismatch makes `total_dictionary_bytes` under-count this table: it
/// adds `distinct_len() * size_of::<Arc<str>>()` and `len() * size_of::<u32>()`,
/// and the `u32` term is always zero, so the index map's own overhead is never
/// counted -- where the QID dictionaries deliberately charge 8 bytes of map slack
/// per entry.
///
/// Not fixed here: this branch may not change production code. The assertion is
/// written as the current behaviour so that fixing it fails this test and forces
/// whoever does it to read why.
#[test]
fn the_statement_fallback_dictionary_reports_zero_entries_while_costing_bytes() {
    let set = set_of(&[full_row()]);
    let fallback = set
        .dictionary_costs()
        .into_iter()
        .find(|(name, _, _)| *name == "statement fallbacks")
        .expect("the dictionary is reported");

    assert_eq!(
        fallback.0, "statement fallbacks",
        "sanity: this is the right row"
    );
    assert_eq!(
        fallback.1, 0,
        "the reported entry count is always zero for this dictionary, because it \
         is read from `ids` and this table never fills `ids`. The correct count is \
         `distinct_len()`, which is 1 here."
    );
    assert!(
        fallback.2 > 0,
        "while its cost is real, which is what makes the pair self-contradictory: {fallback:?}"
    );
    assert_eq!(
        set.statement_fallback_entries(),
        1,
        "and the accessor that answers correctly already reports 1"
    );
}
