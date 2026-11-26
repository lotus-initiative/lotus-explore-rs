// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Query-parameter decoding into [`SearchCriteria`].

use super::super::{QueryParams, is_true_flag};
use super::{CriteriaQueryDto, FormulaQueryDto, PartialNomenclature, RangeF64Dto, RangeU16Dto};
use lotus_model::{ElementState, SearchCriteria, SmilesSearchType};
use std::str::FromStr;

/// Parse search criteria from URL parameters, using `year` as the current year
/// when a parameter is absent.
#[must_use]
pub fn parse_criteria_from_params(params: &QueryParams, year: u16) -> SearchCriteria {
    CriteriaQueryDto::parse(params).into_criteria(year)
}

impl PartialNomenclature {
    fn parse(params: &QueryParams) -> Self {
        Self {
            accepted_synonyms: parse_flag(params, "taxon_accepted_synonyms"),
            basionyms: parse_flag(params, "taxon_basionyms"),
            protonyms: parse_flag(params, "taxon_protonyms"),
            replacements: parse_flag(params, "taxon_replacements"),
        }
    }

    /// Write the specified toggles onto a criteria, leaving the rest alone.
    const fn apply(self, onto: &mut lotus_model::TaxonNomenclature) {
        if let Some(v) = self.accepted_synonyms {
            onto.accepted_synonyms = v;
        }
        if let Some(v) = self.basionyms {
            onto.basionyms = v;
        }
        if let Some(v) = self.protonyms {
            onto.protonyms = v;
        }
        if let Some(v) = self.replacements {
            onto.replacements = v;
        }
    }
}

impl CriteriaQueryDto {
    fn parse(params: &QueryParams) -> Self {
        let taxon = params.get("taxon").cloned();
        let reference = params.get("reference").cloned();
        Self {
            taxon,
            reference,
            taxon_names: PartialNomenclature::parse(params),
            structure: params
                .get("structure")
                .cloned()
                .or_else(|| params.get("smiles").cloned()),
            structure_search_type: params
                .get("structure_search_type")
                .map(String::as_str)
                .or_else(|| params.get("smiles_search_type").map(String::as_str))
                .map(parse_search_type),
            structure_threshold: parse_positive_threshold(
                params
                    .get("similarity_threshold")
                    .or_else(|| params.get("smiles_threshold"))
                    .map(String::as_str),
            ),
            mass_filter: parse_range_when_enabled(params, "mass_filter", "mass_min", "mass_max")
                .map(|r: RangeF64Dto| RangeF64Dto {
                    min: r.min.filter(|v| v.is_finite() && *v >= 0.0),
                    max: r.max.filter(|v| v.is_finite() && *v >= 0.0),
                }),
            year_filter: parse_range_when_enabled(params, "year_filter", "year_start", "year_end"),
            formula_filter: FormulaQueryDto::parse(params),
        }
    }

    fn into_criteria(self, year: u16) -> SearchCriteria {
        let mut criteria = SearchCriteria::up_to_year(year);
        if let Some(taxon) = self.taxon {
            criteria.taxon = taxon;
        }
        if let Some(reference) = self.reference {
            criteria.reference = reference;
        }
        self.taxon_names.apply(&mut criteria.taxon_names);
        if let Some(structure) = self.structure {
            criteria.structure = structure;
        }
        if let Some(search_type) = self.structure_search_type {
            criteria.structure_search = search_type;
        }
        if let Some(threshold) = self.structure_threshold {
            criteria.structure_threshold = threshold;
        }
        if let Some(range) = self.mass_filter {
            range.apply(
                &mut criteria,
                |c, value| c.mass_min = value,
                |c, value| c.mass_max = value,
            );
        }
        if let Some(range) = self.year_filter {
            range.apply(
                &mut criteria,
                |c, value| c.year_min = value,
                |c, value| c.year_max = value,
            );
        }
        if let Some(formula) = self.formula_filter {
            formula.apply(&mut criteria);
        }

        criteria
    }
}

impl FormulaQueryDto {
    fn parse(params: &QueryParams) -> Option<Self> {
        params
            .get("formula_filter")
            .is_some_and(|v| is_true_flag(v))
            .then(|| Self {
                exact: params.get("formula_exact").cloned(),
                c_min: parse_param(params, "c_min"),
                c_max: parse_param(params, "c_max"),
                h_min: parse_param(params, "h_min"),
                h_max: parse_param(params, "h_max"),
                n_min: parse_param(params, "n_min"),
                n_max: parse_param(params, "n_max"),
                o_min: parse_param(params, "o_min"),
                o_max: parse_param(params, "o_max"),
                p_min: parse_param(params, "p_min"),
                p_max: parse_param(params, "p_max"),
                s_min: parse_param(params, "s_min"),
                s_max: parse_param(params, "s_max"),
                f_state: parse_element_state(params, "f_state"),
                cl_state: parse_element_state(params, "cl_state"),
                br_state: parse_element_state(params, "br_state"),
                i_state: parse_element_state(params, "i_state"),
            })
    }

    fn apply(self, criteria: &mut SearchCriteria) {
        // Declarative field assignment: moves value from Option<T> into criteria field when Some.
        macro_rules! set_if_some {
            ($src:expr => $dst:expr) => {
                if let Some(v) = $src {
                    $dst = v;
                }
            };
        }

        criteria.formula_enabled = true;
        set_if_some!(self.exact    => criteria.formula_exact);
        set_if_some!(self.c_min   => criteria.c_min);
        set_if_some!(self.c_max   => criteria.c_max);
        set_if_some!(self.h_min   => criteria.h_min);
        set_if_some!(self.h_max   => criteria.h_max);
        set_if_some!(self.n_min   => criteria.n_min);
        set_if_some!(self.n_max   => criteria.n_max);
        set_if_some!(self.o_min   => criteria.o_min);
        set_if_some!(self.o_max   => criteria.o_max);
        set_if_some!(self.p_min   => criteria.p_min);
        set_if_some!(self.p_max   => criteria.p_max);
        set_if_some!(self.s_min   => criteria.s_min);
        set_if_some!(self.s_max   => criteria.s_max);
        set_if_some!(self.f_state  => criteria.f_state);
        set_if_some!(self.cl_state => criteria.cl_state);
        set_if_some!(self.br_state => criteria.br_state);
        set_if_some!(self.i_state  => criteria.i_state);
    }
}

impl RangeF64Dto {
    fn apply(
        self,
        criteria: &mut SearchCriteria,
        set_min: impl FnOnce(&mut SearchCriteria, f64),
        set_max: impl FnOnce(&mut SearchCriteria, f64),
    ) {
        if let Some(value) = self.min {
            set_min(criteria, value);
        }
        if let Some(value) = self.max {
            set_max(criteria, value);
        }
    }
}

impl RangeU16Dto {
    fn apply(
        self,
        criteria: &mut SearchCriteria,
        set_min: impl FnOnce(&mut SearchCriteria, u16),
        set_max: impl FnOnce(&mut SearchCriteria, u16),
    ) {
        if let Some(value) = self.min {
            set_min(criteria, value);
        }
        if let Some(value) = self.max {
            set_max(criteria, value);
        }
    }
}

/// A boolean parameter, absent when the key is not there at all.
///
/// The distinction from "present and false" is the whole point: absent has to
/// mean "leave the model default alone", or every link written before a toggle
/// existed would silently turn that toggle off.
fn parse_flag(params: &QueryParams, name: &str) -> Option<bool> {
    params.get(name).map(|v| is_true_flag(v))
}

fn parse_param<T: FromStr>(params: &QueryParams, name: &str) -> Option<T> {
    params.get(name).and_then(|value| value.parse::<T>().ok())
}

/// The `(min, max)` of a range parameter pair, if the range is switched on.
///
/// Shared by the two `Range*Dto` types because the rule is the same for both and
/// was written out once per type: a range is absent unless its `enabled` flag is
/// present and true, and the bounds are read whether or not the range is on, so a
/// stale bound cannot leak in from the URL. The element type was the only thing
/// that varied, so it is the only thing that varies here. A third range type is
/// the case where this stops being tidiness and starts paying.
fn parse_range_when_enabled<T, D>(
    params: &QueryParams,
    enabled_key: &str,
    min_key: &str,
    max_key: &str,
) -> Option<D>
where
    T: FromStr,
    D: RangeDto<T>,
{
    params
        .get(enabled_key)
        .is_some_and(|v| is_true_flag(v))
        .then(|| D::from_bounds(parse_param(params, min_key), parse_param(params, max_key)))
}

/// A range parameter pair, built from its bounds.
///
/// The one thing a `Range*Dto` is, so that [`parse_range_when_enabled`] can build
/// whichever one it is asked for. Without this the two call sites each wanted a
/// function returning `(min, max)` and a three-line `map` onto their own type --
/// and those two three-line wrappers were themselves identical, which is what the
/// second round of `cargo echoes` reported. Removing them is the fix; a second
/// helper to deduplicate them would have been the third function for the job.
trait RangeDto<T> {
    /// This DTO over `T`.
    fn from_bounds(min: Option<T>, max: Option<T>) -> Self;
}

impl RangeDto<f64> for RangeF64Dto {
    fn from_bounds(min: Option<f64>, max: Option<f64>) -> Self {
        Self { min, max }
    }
}

impl RangeDto<u16> for RangeU16Dto {
    fn from_bounds(min: Option<u16>, max: Option<u16>) -> Self {
        Self { min, max }
    }
}

fn parse_element_state(params: &QueryParams, name: &str) -> Option<ElementState> {
    params
        .get(name)
        .map(|value| ElementState::from_str(value).unwrap_or_default())
}

/// The model owns the spellings; a second copy here is a second answer to what
/// `stype=fuzzy` means.
fn parse_search_type(value: &str) -> SmilesSearchType {
    SmilesSearchType::parse(value)
}

fn parse_positive_threshold(value: Option<&str>) -> Option<f64> {
    value
        .and_then(|raw| raw.parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0 && *v <= 1.0)
}
