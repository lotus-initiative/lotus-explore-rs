// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `columnar` that need its private fields and functions.

#![allow(clippy::expect_used)]

use super::*;

/// A slot-keyed column holding `cells` in slots `0..cells.len()`.
fn column(cells: &[Option<&str>]) -> SparseStrings {
    let mut strings = SparseStrings::new();
    for (slot, cell) in cells.iter().enumerate() {
        strings.set(slot, *cell);
    }
    strings
}

/// A QID dictionary holding `count` distinct QIDs.
#[cfg(feature = "diagnostics")]
fn qids(count: u32) -> QidDictionary {
    let mut dictionary = QidDictionary::new();
    for numeric in 1..=count {
        dictionary.intern(numeric);
    }
    dictionary
}

#[test]
fn a_qid_dictionary_is_empty_until_something_is_interned() {
    let mut dictionary = QidDictionary::new();
    assert!(dictionary.is_empty());
    dictionary.intern(7);
    assert!(!dictionary.is_empty());
}

#[test]
fn a_sparse_column_counts_its_slots_and_not_its_values() {
    let empty = SparseStrings::new();
    assert_eq!(empty.len(), 0);
    assert!(empty.is_empty());

    // Three slots, two values: `len` is the slot count, which is neither 0, 1 nor
    // the distinct count.
    let filled = column(&[Some("a"), None, Some("a")]);
    assert_eq!(filled.len(), 3);
    assert_eq!(filled.distinct_len(), 1);
    assert!(!filled.is_empty());
}

#[test]
fn a_qid_number_is_only_ascii_digits() {
    // `str::parse::<u32>` accepts a leading `+`, so the digit scan is what refuses
    // it, and it must refuse it whether or not the run is empty.
    assert_eq!(parse_numeric_qid("+12"), None);
    assert_eq!(parse_numeric_qid("1a"), None);
    assert_eq!(parse_numeric_qid(""), None);
    assert_eq!(parse_numeric_qid("12"), Some(12));
}

/// A set whose every dictionary has a different, hand-countable shape.
///
/// Built from fields rather than rows because the builder cannot reach every
/// shape: it never fills `statement_fallbacks`' slots, so that column's slot term
/// would always be zero, and multiplying or dividing a zero changes nothing a
/// test can see. Each count is distinct and above two so that `a * b`, `a + b`
/// and `a / b` all disagree.
#[cfg(feature = "diagnostics")]
fn shaped_set() -> ColumnarResultSet {
    ColumnarResultSet {
        compounds: qids(3),
        // slots, values, string bytes: 4, 2, 5
        compound_names: column(&[Some("ab"), Some("cde"), None, Some("ab")]),
        // 1, 1, 1
        compound_inchikeys: column(&[Some("x")]),
        // 3, 2, 7
        compound_smiles: column(&[Some("CCO"), Some("CCCO"), None]),
        // 6, 3, 10
        compound_formulas: column(&[
            None,
            Some("H2O"),
            Some("H2O"),
            Some("CO2"),
            Some("NaCl"),
            None,
        ]),
        compound_masses: vec![1.0; 4],
        taxa: qids(2),
        // 2, 1, 4
        taxon_names: column(&[Some("Rosa"), Some("Rosa")]),
        references: qids(5),
        // 5, 4, 10
        reference_titles: column(&[Some("A"), Some("BB"), Some("CCC"), Some("DDDD"), None]),
        // 2, 1, 6
        reference_dois: column(&[Some("10.1/x"), None]),
        reference_years: vec![None; 6],
        // 3, 2, 8
        statement_fallbacks: column(&[Some("abc"), Some("defgh"), None]),
        ..ColumnarResultSet::default()
    }
}

#[cfg(feature = "diagnostics")]
#[test]
fn every_dictionary_cost_is_the_documented_arithmetic() {
    // A QID entry is 16 bytes: two u32 and 8 of map slack. A string column is
    // 16 per value (an `Arc<str>`), its string bytes, and 4 per slot.
    assert_eq!(
        shaped_set().dictionary_costs(),
        vec![
            ("compound qids", 3, 48),
            ("compound names", 4, 2 * 16 + 5 + 4 * 4),
            ("compound inchikeys", 1, 16 + 1 + 4),
            ("compound smiles", 3, 2 * 16 + 7 + 3 * 4),
            ("compound formulas", 6, 3 * 16 + 10 + 6 * 4),
            ("compound masses", 4, 32),
            ("taxon qids", 2, 32),
            ("taxon names", 2, 16 + 4 + 2 * 4),
            ("reference qids", 5, 80),
            ("reference titles", 5, 4 * 16 + 10 + 5 * 4),
            ("reference dois", 2, 16 + 6 + 2 * 4),
            ("reference years", 6, 24),
            ("statement fallbacks", 2, 2 * 16 + 8 + 3 * 4),
        ]
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn the_total_is_the_sum_of_the_itemised_costs() {
    let itemised = 48 + 53 + 21 + 51 + 82 + 32 + 32 + 28 + 80 + 94 + 30 + 24 + 52;
    assert_eq!(itemised, 627);
    assert_eq!(shaped_set().total_dictionary_bytes(), 627);
}

#[test]
fn the_empty_fixture_row_is_a_compound_and_nothing_else() {
    // `lotus-query` builds its empty-cell tests on this row; it has to name a
    // compound, or those tests would exercise a row the builder never sees.
    let set = ColumnarResultSet::from_entries(&[crate::testing::empty_row()]);
    assert_eq!(set.compound_qid_text(0).as_deref(), Some("Q42"));
    assert_eq!(set.taxon_qid(0), None);
    assert_eq!(set.reference_qid(0), None);
    assert_eq!(set.inchikey(0), None);
}

#[test]
fn each_compound_takes_exactly_one_slot_in_every_compound_column() {
    let set = ColumnarResultSet::from_entries(&[
        crate::testing::full_row(),
        crate::testing::empty_row(),
        crate::testing::full_row(),
    ]);
    assert_eq!(
        set.compound_masses.len(),
        2,
        "two distinct compounds, not one per row"
    );
    assert_eq!(set.compound_names.len(), 2);
}

#[test]
fn materialising_an_entry_shares_interned_strings_by_arc() {
    let set =
        ColumnarResultSet::from_entries(&[crate::testing::full_row(), crate::testing::full_row()]);
    let first = set.entry(0).expect("row 0");
    let second = set.entry(1).expect("row 1");
    assert!(
        Arc::ptr_eq(&first.name, &second.name),
        "compound name should be Arc-cloned from SparseStrings without reallocating"
    );
    assert!(
        Arc::ptr_eq(&first.taxon_name, &second.taxon_name),
        "taxon name should be Arc-cloned from SparseStrings without reallocating"
    );
    assert!(
        Arc::ptr_eq(
            first.inchikey.as_ref().expect("inchikey"),
            second.inchikey.as_ref().expect("inchikey")
        ),
        "inchikey should be Arc-cloned from SparseStrings without reallocating"
    );
}

#[test]
fn pushing_a_row_normalizes_subscript_digits_in_the_formula() {
    let mut builder = ColumnarBuilder::new();
    builder.push(RawRow {
        compound_qid: "Q1",
        name: "Quercetin",
        inchikey: None,
        smiles: None,
        mass: None,
        formula: Some("C₁₅H₁₀O₇"),
        taxon_qid: "",
        taxon_name: "",
        reference_qid: "",
        reference_node: "",
        ref_title: None,
        ref_doi: None,
        pub_year: None,
        statement: None,
    });
    let set = builder.build();
    assert_eq!(set.formula(0), Some("C15H10O7"));
}
