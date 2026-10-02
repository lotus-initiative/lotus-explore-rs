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
use lotus_model::CompoundEntry;

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
/// cleared. [`CompiledFilters`] is the derived form used per row.
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

/// The filters with their text needles already lowered and trimmed, so testing a
/// row allocates nothing and a blank input matches everything it is next to.
#[derive(Debug, Clone, Default)]
pub struct CompiledFilters {
    compound: String,
    formula: String,
    taxon: String,
    reference: String,
    mass: Option<Range>,
    year: Option<Range>,
}

#[derive(Debug, Clone, Copy)]
struct Range {
    min: Option<f64>,
    max: Option<f64>,
}

impl Range {
    /// Whether `value` falls inside the bounds.
    ///
    /// A missing `value` never matches: see the module comment.
    fn accepts(self, value: Option<f64>) -> bool {
        let Some(value) = value else {
            return false;
        };
        self.min.is_none_or(|min| value >= min) && self.max.is_none_or(|max| value <= max)
    }
}

impl CompiledFilters {
    #[must_use]
    pub fn new(filters: &ColumnFilters) -> Self {
        Self {
            compound: needle(&filters.compound),
            formula: needle(&filters.formula),
            taxon: needle(&filters.taxon),
            reference: needle(&filters.reference),
            mass: range(filters.mass_min, filters.mass_max),
            year: range(filters.year_min, filters.year_max),
        }
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        !self.compound.is_empty()
            || !self.formula.is_empty()
            || !self.taxon.is_empty()
            || !self.reference.is_empty()
            || self.mass.is_some()
            || self.year.is_some()
    }

    /// Whether one row survives every active filter.
    ///
    /// **Across columns, and.** Each filter is a separate thing the user asked
    /// for, so combining them with OR would return rows that fail the constraint
    /// they just typed.
    ///
    /// **Within a column, or.** A column shows several values — a taxon column
    /// shows the name and the QID — and a user who types a QID means that
    /// column, not that particular field. Demanding the text of *every* value in
    /// the column would make searching for a QID return only rows whose name
    /// also contains the QID, which is none of them.
    #[must_use]
    pub fn matches(&self, entry: &CompoundEntry) -> bool {
        any_text(
            &self.compound,
            [
                Some(entry.name.as_ref()),
                Some(entry.compound_qid.as_ref()),
                entry.inchikey.as_deref(),
            ],
        ) && any_text(&self.formula, [entry.formula.as_deref()])
            && any_text(
                &self.taxon,
                [
                    Some(entry.taxon_name.as_ref()),
                    Some(entry.taxon_qid.as_ref()),
                ],
            )
            && any_text(
                &self.reference,
                [
                    Some(entry.reference_qid.as_ref()),
                    entry.ref_title.as_deref(),
                    entry.ref_doi.as_deref(),
                ],
            )
            && self.mass.is_none_or(|range| range.accepts(entry.mass))
            && self
                .year
                .is_none_or(|range| range.accepts(entry.pub_year.map(f64::from)))
    }
}

/// A trimmed, lowercased needle; empty means "no filter on this column".
fn needle(raw: &str) -> String {
    raw.trim().to_lowercase()
}

fn range(min: Option<f64>, max: Option<f64>) -> Option<Range> {
    (min.is_some() || max.is_some()).then_some(Range { min, max })
}

/// Whether `needle` appears in any of a column's values.
///
/// An empty needle is a filter that is not set, so it matches even a row with no
/// value in the column at all. A non-empty needle with nothing to match against
/// is not a match: filtering the formula column by `C15` should not return rows
/// that have no formula.
fn any_text<'a>(needle: &str, fields: impl IntoIterator<Item = Option<&'a str>>) -> bool {
    if needle.is_empty() {
        return true;
    }
    fields
        .into_iter()
        .flatten()
        .any(|field| field.to_lowercase().contains(needle))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
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
            ref_title: Some(Arc::from("Anti-inflammatory activity")),
            ref_doi: Some(Arc::from("10.1000/xyz")),
            pub_year: Some(2019),
            statement: None,
        }
    }

    fn matches(filters: &ColumnFilters, entry: &CompoundEntry) -> bool {
        CompiledFilters::new(filters).matches(entry)
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
