// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The filter set, and the predicates that decide whether a filter is active.

use super::element_max;
use super::{MASS_MAX, YEAR_MIN};
use crate::stats::{ElementState, SmilesSearchType};

/// Every filter the explorer can apply.
///
/// The `has_*` predicates compare against defaults rather than carrying an
/// "active" flag, so a criteria value round-trips through a URL, a JSON body or
/// a command line without needing its provenance.
///
/// The range fields are documented as a group by [`Element_bounds`]: writing
/// `/// Minimum carbon count.` on `c_min` would say less than the field name.
///
/// [`Element_bounds`]: element_max
#[derive(Debug, Clone, PartialEq)]
pub struct SearchCriteria {
    /// Taxon name, scientific name, QID, or `*` for all taxa.
    pub taxon: String,
    /// SMILES or an MDL molfile (V2000/V3000).
    pub structure: String,
    /// How `structure` is matched. [`SmilesSearchType::Substructure`] unless set.
    pub structure_search: SmilesSearchType,
    /// Tanimoto cutoff in 0.0..=1.0, used only for similarity search.
    pub structure_threshold: f64,
    /// Inclusive molecular mass range in daltons; the full span is
    /// 0.0..=[`MASS_MAX`](crate::MASS_MAX).
    pub mass_min: f64,
    /// See [`mass_min`](Self::mass_min).
    pub mass_max: f64,
    /// Inclusive publication year range; the full span is
    /// [`YEAR_MIN`](crate::YEAR_MIN)..=`year_max`.
    pub year_min: u16,
    /// See [`year_min`](Self::year_min).
    pub year_max: u16,
    /// Whether the formula fields below apply at all.
    pub formula_enabled: bool,
    /// Exact formula, compared after subscript-digit normalisation.
    pub formula_exact: String,
    /// Inclusive atom-count ranges for the six filterable elements. Each
    /// `<x>_max` defaults to its [`element_max`] constant, and narrowing either
    /// bound is a filter.
    ///
    /// [`element_max`]: crate::element_max
    #[allow(missing_docs)] // see the note on `f_state` below
    pub c_min: u16,
    #[allow(missing_docs)] // see the note on `f_state` below
    pub c_max: u16,
    #[allow(missing_docs)] // see the note on `f_state` below
    pub h_min: u16,
    #[allow(missing_docs)] // see the note on `f_state` below
    pub h_max: u16,
    #[allow(missing_docs)] // see the note on `f_state` below
    pub n_min: u16,
    #[allow(missing_docs)] // see the note on `f_state` below
    pub n_max: u16,
    #[allow(missing_docs)] // see the note on `f_state` below
    pub o_min: u16,
    #[allow(missing_docs)] // see the note on `f_state` below
    pub o_max: u16,
    #[allow(missing_docs)] // see the note on `f_state` below
    pub p_min: u16,
    #[allow(missing_docs)] // see the note on `f_state` below
    pub p_max: u16,
    #[allow(missing_docs)] // see the note on `f_state` below
    pub s_min: u16,
    #[allow(missing_docs)] // see the note on `f_state` below
    pub s_max: u16,
    /// Presence requirement for each optional halogen.
    ///
    /// `missing_docs` is allowed on this field and on the twelve above it
    /// because all sixteen are the same shape twice -- a bound pair per element,
    /// then a state per halogen -- and documenting each one separately would
    /// write "minimum carbon count" and its eleven variations. The doc comment
    /// on the group above names the convention. Grouping the fields into a
    /// nested struct would read better and is deliberately not done: it is a
    /// change to a type 66 call sites use across five crates, which is a rename
    /// wearing a refactor's coat, not a documentation fix.
    #[allow(missing_docs)]
    pub f_state: ElementState,
    #[allow(missing_docs)] // see the note on `f_state` above
    pub cl_state: ElementState,
    #[allow(missing_docs)] // see the note on `f_state` above
    pub br_state: ElementState,
    #[allow(missing_docs)] // see the note on `f_state` above
    pub i_state: ElementState,
}

impl SearchCriteria {
    /// Criteria bounded by `year_max`, with no other filter set.
    ///
    /// The year is a parameter because this crate does not read a clock: a
    /// caller that wants "up to now" passes the year it got from the platform.
    #[must_use]
    pub const fn up_to_year(year_max: u16) -> Self {
        Self {
            taxon: String::new(),
            structure: String::new(),
            structure_search: SmilesSearchType::Substructure,
            structure_threshold: 0.8,
            mass_min: 0.0,
            mass_max: MASS_MAX,
            year_min: YEAR_MIN,
            year_max,
            formula_enabled: false,
            formula_exact: String::new(),
            c_min: 0,
            c_max: element_max::C,
            h_min: 0,
            h_max: element_max::H,
            n_min: 0,
            n_max: element_max::N,
            o_min: 0,
            o_max: element_max::O,
            p_min: 0,
            p_max: element_max::P,
            s_min: 0,
            s_max: element_max::S,
            f_state: ElementState::Allowed,
            cl_state: ElementState::Allowed,
            br_state: ElementState::Allowed,
            i_state: ElementState::Allowed,
        }
    }

    /// The six filterable elements as `(symbol, min, max, default_max)`.
    #[must_use]
    pub const fn element_ranges(&self) -> [(&'static str, u16, u16, u16); 6] {
        [
            ("C", self.c_min, self.c_max, element_max::C),
            ("H", self.h_min, self.h_max, element_max::H),
            ("N", self.n_min, self.n_max, element_max::N),
            ("O", self.o_min, self.o_max, element_max::O),
            ("P", self.p_min, self.p_max, element_max::P),
            ("S", self.s_min, self.s_max, element_max::S),
        ]
    }

    /// The four optional halogens as `(symbol, state)`.
    #[must_use]
    pub const fn halogen_states(&self) -> [(&'static str, ElementState); 4] {
        [
            ("F", self.f_state),
            ("Cl", self.cl_state),
            ("Br", self.br_state),
            ("I", self.i_state),
        ]
    }

    /// Whether the mass range is narrower than the full 0..=`MASS_MAX` span.
    #[must_use]
    pub fn has_mass_filter(&self) -> bool {
        self.mass_min > 0.0 || self.mass_max < MASS_MAX
    }

    /// Whether the year range is narrower than `year_max`.
    ///
    /// Takes the year it is comparing against rather than reading a clock, so
    /// that a serialised criteria does not silently change meaning.
    #[must_use]
    pub const fn has_year_filter(&self, year_max: u16) -> bool {
        self.year_min > YEAR_MIN || self.year_max < year_max
    }

    /// Whether formula filtering applies, which needs both the flag and a value
    /// that is not the default.
    #[must_use]
    pub fn has_formula_filter(&self) -> bool {
        if !self.formula_enabled {
            return false;
        }
        if !self.formula_exact.trim().is_empty() {
            return true;
        }
        if self
            .element_ranges()
            .iter()
            .any(|(_, min, max, default_max)| *min > 0 || *max < *default_max)
        {
            return true;
        }
        self.halogen_states()
            .iter()
            .any(|(_, state)| *state != ElementState::Allowed)
    }

    /// Whether any filter beyond taxon is narrowing the result set.
    #[must_use]
    pub fn has_effective_filters(&self, year_max: u16) -> bool {
        !self.structure.trim().is_empty()
            || self.has_mass_filter()
            || self.has_year_filter(year_max)
            || self.has_formula_filter()
    }

    /// Whether this criteria would send a query at all.
    #[must_use]
    pub fn is_searchable(&self) -> bool {
        !self.taxon.trim().is_empty() || !self.structure.trim().is_empty()
    }
}

#[cfg(test)]
#[path = "criteria/tests.rs"]
mod tests;
