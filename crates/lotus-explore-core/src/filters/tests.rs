// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `filters`, in their own file.

#![allow(clippy::expect_used)]

use super::*;
use lotus_model::{ColumnarResultSet, CompoundEntry};
use std::sync::Arc;

fn entry() -> CompoundEntry {
    CompoundEntry {
        compound_qid: Arc::from("Q1"),
        name: Arc::from("Gentianine"),
        inchikey: Some(Arc::from("BEHABCJQKGGRQN-XLPZGREQSA-N")),
        smiles: None,
        mass: Some(250.25),
        formula: Some(Arc::from("C15H10O5")),
        taxon_qid: Arc::from("Q2598745"),
        taxon_name: Arc::from("Gentiana lutea"),
        reference_qid: Arc::from("Q999"),
        reference_node: Arc::from(""),
        ref_title: Some(Arc::from("Anti-inflammatory activity")),
        ref_doi: Some(Arc::from("10.1000/xyz")),
        pub_year: Some(2019),
        statement: None,
    }
}

/// Whether the filter admits `entry`.
///
/// Goes through a one-row columnar set, which is the only path a filter takes
/// now. A helper that called a predicate directly would keep testing a code
/// path the app no longer has.
fn matches(filters: &ColumnFilters, entry: &CompoundEntry) -> bool {
    let set = ColumnarResultSet::from_entries(std::slice::from_ref(entry));
    set.plan_filter(&filters.to_spec()).accepts(&set, 0)
}

#[test]
fn no_filters_match_everything() {
    assert!(!ColumnFilters::empty().is_active());
    assert!(matches(&ColumnFilters::empty(), &entry()));
}

#[test]
fn a_blank_input_is_not_a_filter() {
    let filters = ColumnFilters {
        compound: "   ".to_owned(),
        ..ColumnFilters::empty()
    };
    assert!(!filters.is_active());
    assert_eq!(filters.active_count(), 0);
    assert!(matches(&filters, &entry()));
}

#[test]
fn text_filters_match_names_ignoring_case_and_substring() {
    let filters = ColumnFilters {
        taxon: "LUTEA".to_owned(),
        ..ColumnFilters::empty()
    };
    assert_eq!(filters.active_count(), 1);
    assert!(matches(&filters, &entry()));

    let filters = ColumnFilters {
        compound: "gent".to_owned(),
        ..ColumnFilters::empty()
    };
    assert!(matches(&filters, &entry()));
}

#[test]
fn text_filters_reach_the_identifiers_shown_beside_the_name() {
    // A QID is one of the values the taxon column shows, so typing it
    // filters that column — it does not also have to appear in the name.
    let filters = ColumnFilters {
        taxon: "q2598745".to_owned(),
        ..ColumnFilters::empty()
    };
    assert!(matches(&filters, &entry()));

    let filters = ColumnFilters {
        reference: "10.1000".to_owned(),
        ..ColumnFilters::empty()
    };
    assert!(matches(&filters, &entry()));

    let filters = ColumnFilters {
        compound: "BEHABCJ".to_owned(),
        ..ColumnFilters::empty()
    };
    assert!(matches(&filters, &entry()));
}

#[test]
fn columns_combine_with_and_so_a_row_must_satisfy_all_of_them() {
    let filters = ColumnFilters {
        taxon: "Gentiana".to_owned(),
        compound: "Gentianine".to_owned(),
        ..ColumnFilters::empty()
    };
    assert!(matches(&filters, &entry()));

    let conflicting = ColumnFilters {
        taxon: "Gentiana".to_owned(),
        compound: "Quercetin".to_owned(),
        ..ColumnFilters::empty()
    };
    assert!(!matches(&conflicting, &entry()));
}

#[test]
fn a_text_filter_rejects_a_row_that_does_not_contain_it() {
    let filters = ColumnFilters {
        taxon: "Rosa".to_owned(),
        ..ColumnFilters::empty()
    };
    assert!(!matches(&filters, &entry()));
}

#[test]
fn text_filters_and_ranges_combine_with_and() {
    let filters = ColumnFilters {
        taxon: "Gentiana".to_owned(),
        mass_min: Some(200.0),
        year_max: Some(2020.0),
        ..ColumnFilters::empty()
    };
    assert_eq!(filters.active_count(), 3);
    assert!(matches(&filters, &entry()));

    let narrowed = ColumnFilters {
        taxon: "Gentiana".to_owned(),
        mass_min: Some(300.0),
        ..ColumnFilters::empty()
    };
    assert!(!matches(&narrowed, &entry()));
}

#[test]
fn numeric_filters_are_inclusive_at_both_ends() {
    for bounds in [
        (Some(250.25), None),
        (None, Some(250.25)),
        (Some(250.25), Some(250.25)),
    ] {
        let filters = ColumnFilters {
            mass_min: bounds.0,
            mass_max: bounds.1,
            ..ColumnFilters::empty()
        };
        assert!(
            matches(&filters, &entry()),
            "a bound equal to the value must include it: {bounds:?}"
        );
    }
}

#[test]
fn a_numeric_filter_excludes_rows_that_have_no_value() {
    let mut unknown = entry();
    unknown.mass = None;
    let filters = ColumnFilters {
        mass_max: Some(1000.0),
        ..ColumnFilters::empty()
    };
    assert!(
        !matches(&filters, &unknown),
        "an unknown mass is not a small mass"
    );

    let mut undated = entry();
    undated.pub_year = None;
    let filters = ColumnFilters {
        year_min: Some(1000.0),
        ..ColumnFilters::empty()
    };
    assert!(!matches(&filters, &undated));
}

#[test]
fn a_filter_on_a_value_the_row_does_not_have_never_matches() {
    let mut no_formula = entry();
    no_formula.formula = None;

    // Filtering by formula cannot return a row that has no formula.
    let filters = ColumnFilters {
        formula: "C15".to_owned(),
        ..ColumnFilters::empty()
    };
    assert!(!matches(&filters, &no_formula));

    // But a name filter is about the name, and an absent formula says
    // nothing about it.
    let filters = ColumnFilters {
        compound: "gentianine".to_owned(),
        ..ColumnFilters::empty()
    };
    assert!(matches(&filters, &no_formula));
}

#[test]
fn clearing_forgets_everything() {
    let mut filters = ColumnFilters {
        compound: "a".to_owned(),
        mass_min: Some(1.0),
        ..ColumnFilters::empty()
    };
    assert!(filters.is_active());

    filters.clear();

    assert!(!filters.is_active());
    assert_eq!(filters.active_count(), 0);
}

#[test]
fn active_count_counts_columns_not_keys() {
    let filters = ColumnFilters {
        mass_min: Some(1.0),
        mass_max: Some(2.0),
        year_min: Some(1.0),
        year_max: Some(2.0),
        ..ColumnFilters::empty()
    };
    assert_eq!(filters.active_count(), 2);
}
