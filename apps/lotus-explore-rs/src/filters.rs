// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Client-side filtering of an already-fetched result set.
//!
//! The query runs against Wikidata; this is the second, narrower pass over what
//! came back. It exists because a SPARQL round trip per keystroke is a bad trade
//! for "show me the rows whose taxon name contains `ros`", and because the
//! result set is already in memory — filtering it costs a linear scan and no
//! network.
//!
//! **The whole result set is in memory.** The fetch asks the endpoint for every
//! row rather than for one screen's worth, so a filter answers a question about
//! the result set rather than about the first 500 rows of it. See
//! [`ColumnFilters::to_spec`] for how the filters are handed to the columnar
//! store.
//!
//! Each column gets the kind of control its data is:
//!
//! - names and formulas are matched as text, case-insensitively, on substring;
//! - masses and years are matched numerically against a range.
//!
//! A numeric filter **excludes rows with no value** for that column. That is the
//! only defensible reading: a row with an unknown mass is not evidence of a mass
//! below 200, so it cannot satisfy "at most 200". Treating a missing value as
//! zero would quietly put every unreported compound in the low-mass bucket.

use crate::sort::SortColumn;
use lotus_model::{FilterSpec, Range};

/// The control a column's filter needs, which follows from what the column holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    /// Free text, matched as a case-insensitive substring.
    Text,
    /// A numeric range, one bound or both.
    Range,
}

impl FilterKind {
    /// Which control a column gets. Decided here, once, from what the column
    /// holds — the header list and the filter row both ask this, so a column
    /// cannot end up with a text box over a column of numbers.
    #[must_use]
    pub const fn for_column(column: SortColumn) -> Self {
        match column {
            SortColumn::Mass | SortColumn::PubYear => Self::Range,
            SortColumn::Name
            | SortColumn::Formula
            | SortColumn::TaxonName
            | SortColumn::RefTitle => Self::Text,
        }
    }
}

/// Every column's filter, as the user typed it.
///
/// Kept as the raw strings and numbers the inputs hold, not as a compiled
/// predicate, because this is also what the inputs are rendered from — a control
/// whose value is not readable back out of state is a control that cannot be
/// cleared. [`to_spec`](ColumnFilters::to_spec) is the derived form the columnar
/// store compiles.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ColumnFilters {
    pub compound: String,
    pub formula: String,
    pub taxon: String,
    pub reference: String,
    pub mass_min: Option<f64>,
    pub mass_max: Option<f64>,
    pub year_min: Option<f64>,
    pub year_max: Option<f64>,
}

impl ColumnFilters {
    /// No filters set, which is the state a new search starts from.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            compound: String::new(),
            formula: String::new(),
            taxon: String::new(),
            reference: String::new(),
            mass_min: None,
            mass_max: None,
            year_min: None,
            year_max: None,
        }
    }

    /// Forget every filter.
    pub fn clear(&mut self) {
        *self = Self::empty();
    }

    /// Whether anything is filtering, so the table can skip the pass entirely.
    ///
    /// Asked of the same normalised values the row test uses, so a box holding
    /// nothing but spaces counts as no filter rather than as a filter that
    /// matches nothing.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active_count() > 0
    }

    /// The filters as the columnar store wants them.
    ///
    /// This is the whole of the app's filtering contract now. The store compiles
    /// a spec into one bitmap per constrained dictionary, so the per-row work is
    /// four bit tests rather than a substring search over thirteen fields — which
    /// is what lets a keystroke scan millions of rows instead of hundreds.
    ///
    /// A blank text box becomes an empty needle, which is not a filter -- the same
    /// rule the row-at-a-time predicate applied, kept because it is the rule the
    /// filter row's controls were built against.
    #[must_use]
    pub fn to_spec(&self) -> FilterSpec {
        FilterSpec {
            compound: self.compound.trim().to_owned(),
            formula: self.formula.trim().to_owned(),
            taxon: self.taxon.trim().to_owned(),
            reference: self.reference.trim().to_owned(),
            mass: range_spec(self.mass_min, self.mass_max),
            year: range_spec(self.year_min, self.year_max),
        }
    }

    /// How many columns are filtering, for a "3 filters" affordance.
    #[must_use]
    pub fn active_count(&self) -> usize {
        [
            !self.compound.trim().is_empty(),
            !self.formula.trim().is_empty(),
            !self.taxon.trim().is_empty(),
            !self.reference.trim().is_empty(),
            self.mass_min.is_some() || self.mass_max.is_some(),
            self.year_min.is_some() || self.year_max.is_some(),
        ]
        .into_iter()
        .filter(|active| *active)
        .count()
    }

    /// The text this column filters by. `&""` for the columns that do not take
    /// text, so a control can read its own value without asking which kind it is.
    #[must_use]
    pub fn text(&self, column: SortColumn) -> &str {
        match column {
            SortColumn::Name => &self.compound,
            SortColumn::Formula => &self.formula,
            SortColumn::TaxonName => &self.taxon,
            SortColumn::RefTitle => &self.reference,
            SortColumn::Mass | SortColumn::PubYear => "",
        }
    }

    /// Set a column's text filter. Ignored for the numeric columns.
    pub fn set_text(&mut self, column: SortColumn, value: String) {
        match column {
            SortColumn::Name => self.compound = value,
            SortColumn::Formula => self.formula = value,
            SortColumn::TaxonName => self.taxon = value,
            SortColumn::RefTitle => self.reference = value,
            SortColumn::Mass | SortColumn::PubYear => {}
        }
    }

    /// The low bound on a numeric column's filter.
    #[must_use]
    pub fn min(&self, column: SortColumn) -> Option<f64> {
        match column {
            SortColumn::Mass => self.mass_min,
            SortColumn::PubYear => self.year_min,
            _ => None,
        }
    }

    /// The high bound on a numeric column's filter.
    #[must_use]
    pub fn max(&self, column: SortColumn) -> Option<f64> {
        match column {
            SortColumn::Mass => self.mass_max,
            SortColumn::PubYear => self.year_max,
            _ => None,
        }
    }

    /// Set one bound on a numeric column's filter.
    pub fn set_bound(&mut self, column: SortColumn, bound: Bound, value: Option<f64>) {
        match (column, bound) {
            (SortColumn::Mass, Bound::Min) => self.mass_min = value,
            (SortColumn::Mass, Bound::Max) => self.mass_max = value,
            (SortColumn::PubYear, Bound::Min) => self.year_min = value,
            (SortColumn::PubYear, Bound::Max) => self.year_max = value,
            _ => {}
        }
    }
}

/// Which end of a numeric range a bound is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bound {
    Min,
    Max,
}

/// A range over the columnar store, or `None` when neither bound is set.
///
/// `None` for "neither" rather than for "a range with no bounds", because the
/// second would match every row and silently do nothing the reader asked for.
fn range_spec(min: Option<f64>, max: Option<f64>) -> Option<Range> {
    (min.is_some() || max.is_some()).then_some(Range { min, max })
}

#[cfg(test)]
mod tests {
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
}
