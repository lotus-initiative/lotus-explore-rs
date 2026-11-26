// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Case-folded substring matching and compiled bitmap filters over [`ColumnarResultSet`].

use super::bitmask::{Bitmask, build_mask};
use super::qid_dict::{QID_BUFFER, qid_matches, slot_to_id};
use super::{ColumnarResultSet, NO_VALUE};

impl ColumnarResultSet {
    /// Compile a filter against this set.
    ///
    /// The returned [`FilterPlan`] holds one bitmap per constrained dictionary.
    /// A row is accepted when its four ids pass, which turns a keystroke into a
    /// pass over the dictionaries plus four bit tests per row.
    #[must_use]
    pub fn plan_filter(&self, spec: &FilterSpec) -> FilterPlan {
        let mut plan = FilterPlan::default();

        if !spec.compound.trim().is_empty() {
            let needle = folded(&spec.compound);
            let mut buffer = [0u8; QID_BUFFER];
            plan.compound = Some(build_mask(self.compounds.len(), |slot, id| {
                // The QID arm renders through the reusable stack buffer rather than
                // allocating a String per compound per keystroke.
                self.compound_names
                    .get(slot)
                    .is_some_and(|n| contains_folded(n, &needle))
                    || self
                        .compound_inchikeys
                        .get(slot)
                        .is_some_and(|k| contains_folded(k, &needle))
                    // Through `get`, so `qid_matches` is handed the compound's QID
                    // rather than the position it occupies. Ids here are slots --
                    // positions in first-seen order -- so passing `id` directly
                    // tested *which slot a compound landed on*: a filter for
                    // Q3613679 matched nothing unless the compound's name happened
                    // to contain that text, and a filter for "Q1" matched whichever
                    // compound sat at slot 1. The taxon filter below has always
                    // done this translation; this arm did not.
                    || self
                        .compounds
                        .get(id)
                        .is_some_and(|numeric| qid_matches(numeric, &needle, &mut buffer))
            }));
        }

        if !spec.formula.trim().is_empty() {
            let normalized = crate::normalize_formula_subscripts(&spec.formula);
            let needle = folded(&normalized);
            plan.formula = Some(build_mask(self.compounds.len(), |slot, _id| {
                self.compound_formulas
                    .get(slot)
                    .is_some_and(|f| contains_folded(f, &needle))
            }));
        }

        if let Some(range) = spec.mass {
            plan.mass = Some(build_mask(self.compounds.len(), |_slot, id| {
                range.accepts(self.compound_mass(id))
            }));
        }

        if !spec.taxon.trim().is_empty() {
            let needle = folded(&spec.taxon);
            let mut buffer = [0u8; QID_BUFFER];
            plan.taxon = Some(build_mask(self.taxa.len(), |slot, id| {
                self.taxa
                    .get(id)
                    .is_some_and(|numeric| qid_matches(numeric, &needle, &mut buffer))
                    || self
                        .taxon_names
                        .get(slot)
                        .is_some_and(|n| contains_folded(n, &needle))
            }));
        }

        if !spec.reference.trim().is_empty() {
            let needle = folded(&spec.reference);
            let mut buffer = [0u8; QID_BUFFER];
            plan.reference = Some(build_mask(self.references.len(), |slot, id| {
                self.references
                    .get(id)
                    .is_some_and(|numeric| qid_matches(numeric, &needle, &mut buffer))
                    || self
                        .reference_titles
                        .get(slot)
                        .is_some_and(|t| contains_folded(t, &needle))
                    || self
                        .reference_dois
                        .get(slot)
                        .is_some_and(|d| contains_folded(d, &needle))
            }));
        }

        if let Some(range) = spec.year {
            plan.year = Some(build_mask(self.references.len(), |_slot, id| {
                // An absent year never matches: an unknown year is not a recent
                // one, and the same rule the mass range follows.
                range.accepts(self.reference_year(id).map(f64::from))
            }));
        }

        plan
    }
}

/// The characters of `text` under Unicode lowercasing.
///
/// Folded once per filter, not once per field per row. This is the allocation
/// the old filter made six to nine times per row per keystroke.
#[must_use]
pub fn folded(text: &str) -> Vec<char> {
    text.trim().to_lowercase().chars().collect()
}

/// Whether `haystack` contains `needle`, ignoring case, without allocating.
///
/// `needle` must already be [`folded`]. The haystack is folded as it is walked,
/// which is why this costs nothing per row: lowercasing both sides into fresh
/// `String`s was the dominant cost of filtering at all.
#[must_use]
pub fn contains_folded(haystack: &str, needle: &[char]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.is_ascii() {
        if !needle.iter().all(char::is_ascii) {
            return false;
        }
        return haystack.as_bytes().windows(needle.len()).any(|window| {
            window
                .iter()
                .zip(needle)
                .all(|(&b, &n)| char::from(b.to_ascii_lowercase()) == n)
        });
    }
    let mut rest = haystack;
    while !rest.is_empty() {
        if folded_prefix_matches(rest, needle) {
            return true;
        }
        // `map_or` rather than `unwrap`: an empty `rest` cannot reach here, but
        // the lint profile does not take the reader's word for it.
        let width = rest.chars().next().map_or(1, char::len_utf8);
        rest = rest.get(width..).unwrap_or_default();
    }
    false
}

/// Whether the folded prefix of `haystack` is `needle`.
fn folded_prefix_matches(haystack: &str, needle: &[char]) -> bool {
    let mut folded_haystack = haystack.chars().flat_map(char::to_lowercase);
    needle
        .iter()
        .all(|&expected| folded_haystack.next() == Some(expected))
}

/// An inclusive range on a measured quantity, where either end may be open.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Range {
    /// The lower bound, if the range has one.
    pub min: Option<f64>,
    /// The upper bound, if the range has one.
    pub max: Option<f64>,
}

impl Range {
    /// Whether `value` falls in the range.
    ///
    /// A missing `value` never matches. "An unknown mass is not a small mass":
    /// the alternative makes `mass <= 200` return every compound Wikidata has
    /// not weighed, which is most of them.
    #[must_use]
    pub fn accepts(self, value: Option<f64>) -> bool {
        let Some(value) = value else { return false };
        self.min.is_none_or(|min| value >= min) && self.max.is_none_or(|max| value <= max)
    }
}

/// A filter, as the table's filter row states it.
///
/// The needles are raw user input; folding happens once per compile, in
/// [`ColumnarResultSet::plan_filter`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FilterSpec {
    /// Substring to find in a compound's name, QID or `InChIKey`.
    pub compound: String,
    /// Substring to find in a compound's formula.
    pub formula: String,
    /// Substring to find in a taxon's QID or scientific name.
    pub taxon: String,
    /// Substring to find in a reference's QID, title or DOI.
    pub reference: String,
    /// The mass window, if the row asks for one.
    pub mass: Option<Range>,
    /// The publication-year window, if the row asks for one.
    pub year: Option<Range>,
}

impl FilterSpec {
    /// Whether any column is constrained.
    #[must_use]
    pub fn is_active(&self) -> bool {
        !self.compound.trim().is_empty()
            || !self.formula.trim().is_empty()
            || !self.taxon.trim().is_empty()
            || !self.reference.trim().is_empty()
            || self.mass.is_some()
            || self.year.is_some()
    }
}

/// A compiled filter: one bitmap per constrained dictionary.
///
/// Four bit tests per row, and no string comparison per row at all.
#[derive(Debug, Clone, Default)]
pub struct FilterPlan {
    compound: Option<Bitmask>,
    formula: Option<Bitmask>,
    mass: Option<Bitmask>,
    taxon: Option<Bitmask>,
    reference: Option<Bitmask>,
    year: Option<Bitmask>,
}

impl FilterPlan {
    /// Whether the plan constrains anything.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.compound.is_some()
            || self.formula.is_some()
            || self.mass.is_some()
            || self.taxon.is_some()
            || self.reference.is_some()
            || self.year.is_some()
    }

    /// Whether row `row` of `set` survives.
    #[must_use]
    pub fn accepts(&self, set: &ColumnarResultSet, row: usize) -> bool {
        let Some(&compound) = set.compound_ids.get(row) else {
            return false;
        };
        let taxon = set.taxon_ids.get(row).copied().unwrap_or(NO_VALUE);
        let reference = set.reference_ids.get(row).copied().unwrap_or(NO_VALUE);

        passes(self.compound.as_ref(), compound)
            && passes(self.formula.as_ref(), compound)
            && passes(self.mass.as_ref(), compound)
            && passes(self.taxon.as_ref(), taxon)
            && passes(self.reference.as_ref(), reference)
            && passes(self.year.as_ref(), reference)
    }

    /// The ids of every row that survives, in row order.
    ///
    /// The table's filtered view is this list, so it is built once per filter
    /// change and then read by the sort and the virtualiser.
    #[must_use]
    pub fn surviving_rows(&self, set: &ColumnarResultSet) -> Vec<u32> {
        (0..set.row_count())
            .filter(|row| self.accepts(set, *row))
            .map(slot_to_id)
            .collect()
    }

    /// How many rows survive.
    #[must_use]
    pub fn surviving_count(&self, set: &ColumnarResultSet) -> usize {
        (0..set.row_count())
            .filter(|row| self.accepts(set, *row))
            .count()
    }
}

/// An optional mask that passes everything when it is absent.
fn passes(mask: Option<&Bitmask>, id: u32) -> bool {
    mask.is_none_or(|m| m.contains(id))
}
