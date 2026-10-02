// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The filter set, and the predicates that decide whether a filter is active.

use super::element_max;
use super::{MASS_MAX, YEAR_MIN};
use crate::stats::{DEFAULT_STRUCTURE_THRESHOLD, ElementState, SmilesSearchType};

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
    /// A Wikidata QID or a DOI, naming the reference whose reported compounds to
    /// show. Empty means no reference constraint.
    ///
    /// Resolved to a QID before the query is built, the way `taxon` is. Not a
    /// title: a title is prose, and matching prose against 400,000 references is
    /// a question with no useful answer — but a DOI and a QID both name exactly
    /// one item.
    pub reference: String,
    /// Which nomenclatural relationships a taxon search follows, beyond the
    /// taxon name as typed. All four default to `true`.
    ///
    /// A nested struct rather than four loose booleans: they are one decision —
    /// *how far back in this taxon's naming history to look* — they are always
    /// read together, and `clippy::struct_excessive_bools` is right that five
    /// unrelated `bool`s in one flat struct is a shape that grows badly. See
    /// [`crate::taxon_nomenclature`] for what each relationship means and why
    /// they are four and not one.
    pub taxon_names: TaxonNomenclature,
    /// SMILES or an MDL molfile (V2000/V3000).
    pub structure: String,
    /// How `structure` is matched. [`SmilesSearchType::Similarity`] unless set.
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
    // Each `<x>_max` defaults to its [`element_max`] constant, and narrowing
    // either bound is a filter; an unbound maximum means "no constraint".
    //
    // [`element_max`]: crate::element_max
    /// Lower bound on the atom count of carbon.
    pub c_min: u16,
    /// Upper bound on the atom count of carbon.
    pub c_max: u16,
    /// Lower bound on the atom count of hydrogen.
    pub h_min: u16,
    /// Upper bound on the atom count of hydrogen.
    pub h_max: u16,
    /// Lower bound on the atom count of nitrogen.
    pub n_min: u16,
    /// Upper bound on the atom count of nitrogen.
    pub n_max: u16,
    /// Lower bound on the atom count of oxygen.
    pub o_min: u16,
    /// Upper bound on the atom count of oxygen.
    pub o_max: u16,
    /// Lower bound on the atom count of phosphorus.
    pub p_min: u16,
    /// Upper bound on the atom count of phosphorus.
    pub p_max: u16,
    /// Lower bound on the atom count of sulfur.
    pub s_min: u16,
    /// Upper bound on the atom count of sulfur.
    pub s_max: u16,
    // Halogens are optional, so each is a presence requirement rather than
    // a count: a compound has fluorine or it does not, and there is no
    // meaningful "one and a half fluorines".
    /// Whether fluorine must be present, must be absent, or is unconstrained.
    pub f_state: ElementState,
    /// Whether chlorine must be present, must be absent, or is unconstrained.
    pub cl_state: ElementState,
    /// Whether bromine must be present, must be absent, or is unconstrained.
    pub br_state: ElementState,
    /// Whether iodine must be present, must be absent, or is unconstrained.
    pub i_state: ElementState,
}

/// Which of the four nomenclatural relationships a taxon search follows.
///
/// Every field defaults to `true`, because a search that quietly returns fewer
/// compounds than the literature holds is the failure mode this exists to
/// prevent — see [`crate::taxon_nomenclature`] for the relationships and
/// `docs/TAXON-SEARCH.md` for worked examples.
/// Which nomenclatural relationships a taxon search follows.
///
/// Four independent toggles, and that is the shape they have to be: each names a
/// distinct relationship, each can be on with the other three off, and the state
/// space is genuinely 2^4. The lint's own suggestions -- a state machine, or a
/// two-variant enum per field -- would replace four readable `bool`s with sixteen
/// named states and no way to say "all of them", which is the value a caller
/// reaches for most often. [`ALL_ON`](Self::ALL_ON) is that expression.
#[allow(
    clippy::struct_excessive_bools,
    reason = "four independent toggles over a 16-state space; a state machine would have no way to say `ALL_ON`"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaxonNomenclature {
    /// Accepted name ↔ its synonyms (`P1420` / `P12763`).
    ///
    /// Example: *Leontopodium nivale* carries `P1420` to its accepted name
    /// *Leontopodium alpinum*, which holds 33 compounds and *nivale* none.
    /// Not chronological — either name may be the older one.
    pub accepted_synonyms: bool,
    /// New combination ↔ its basionym (`P566` / `P12766`).
    ///
    /// Example: *Houpoea officinalis* is a new combination for *Magnolia
    /// officinalis*, and all 226 LOTUS compounds are filed under the basionym.
    pub basionyms: bool,
    /// Current name ↔ its original combination, whose zoological mirror image
    /// is a protonym (`P1403` / `P12765`).
    ///
    /// Example: *Gonyaulax tamarensis* is the original combination of
    /// *Alexandrium tamarense*.
    pub protonyms: bool,
    /// Replacement name (*nomen novum*) ↔ the name it replaced
    /// (`P694` / `P12764`).
    ///
    /// Example: *Salvia rosmarinus* replaced *Rosmarinus officinalis*, and the
    /// 595 compounds under the old name and the 31 under the new one are
    /// compounds from one plant.
    pub replacements: bool,
}

impl TaxonNomenclature {
    /// Every relationship followed. The default.
    pub const ALL_ON: Self = Self {
        accepted_synonyms: true,
        basionyms: true,
        protonyms: true,
        replacements: true,
    };

    /// No relationship followed: the taxon name as typed, and nothing else.
    pub const ALL_OFF: Self = Self {
        accepted_synonyms: false,
        basionyms: false,
        protonyms: false,
        replacements: false,
    };

    /// Whether any relationship is followed.
    #[must_use]
    pub const fn any(&self) -> bool {
        self.accepted_synonyms || self.basionyms || self.protonyms || self.replacements
    }

    /// Set the value for a given relationship.
    pub const fn set_for_relation(
        &mut self,
        relation: &crate::taxon_nomenclature::Relation,
        on: bool,
    ) {
        use crate::taxon_nomenclature as n;
        if relation.slot == n::ACCEPTED_SYNONYM.slot {
            self.accepted_synonyms = on;
        } else if relation.slot == n::BASIONYM.slot {
            self.basionyms = on;
        } else if relation.slot == n::PROTONYM.slot {
            self.protonyms = on;
        } else {
            self.replacements = on;
        }
    }

    /// The value for a given relationship.
    ///
    /// Dispatching on `Relation::slot` rather than `Relation::id`: an `id` is
    /// a `&'static str`, and a const path in a match arm sits in pattern
    /// position, where only a literal or a binding is allowed. The `slot`
    /// exists for exactly this.
    #[must_use]
    pub const fn for_relation(&self, relation: &crate::taxon_nomenclature::Relation) -> bool {
        use crate::taxon_nomenclature as n;
        if relation.slot == n::ACCEPTED_SYNONYM.slot {
            self.accepted_synonyms
        } else if relation.slot == n::BASIONYM.slot {
            self.basionyms
        } else if relation.slot == n::PROTONYM.slot {
            self.protonyms
        } else {
            self.replacements
        }
    }
}

impl Default for TaxonNomenclature {
    fn default() -> Self {
        Self::ALL_ON
    }
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
            reference: String::new(),
            taxon_names: TaxonNomenclature::ALL_ON,
            structure: String::new(),
            structure_search: SmilesSearchType::Exact,
            structure_threshold: DEFAULT_STRUCTURE_THRESHOLD,
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

    /// Whether any nomenclatural relationship is being followed.
    #[must_use]
    pub const fn has_nomenclatural_relations(&self) -> bool {
        self.taxon_names.any()
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
