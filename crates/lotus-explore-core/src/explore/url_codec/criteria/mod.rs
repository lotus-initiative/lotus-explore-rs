// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! URL query-parameter DTO and bidirectional codec for [`lotus_model::SearchCriteria`].

mod decode;
mod encode;

pub use decode::parse_criteria_from_params;
pub use encode::criteria_query_params;

use lotus_model::{ElementState, SmilesSearchType};

#[derive(Clone, Debug, Default, PartialEq)]
struct CriteriaQueryDto {
    taxon: Option<String>,
    /// A Wikidata QID or a DOI, as typed. `Option` because absent has to stay
    /// absent: a link written before the field existed must not arrive with an
    /// empty-but-present reference.
    reference: Option<String>,
    /// The four nomenclatural toggles, `None` when a parameter is absent, so an
    /// old shared link parses to the model defaults rather than to a search
    /// that turns things off nobody asked it to.
    taxon_names: PartialNomenclature,
    structure: Option<String>,
    structure_search_type: Option<SmilesSearchType>,
    structure_threshold: Option<f64>,
    mass_filter: Option<RangeF64Dto>,
    year_filter: Option<RangeU16Dto>,
    formula_filter: Option<FormulaQueryDto>,
}

/// The four nomenclatural toggles, each still possibly unspecified.
///
/// A parallel `Option`-per-field shape rather than a `TaxonNomenclature` with
/// a default, because the two states are genuinely different and both are
/// reachable: *absent* means "leave the model default alone", while *present
/// and false* means "the user turned this off". Collapsing them into a
/// `TaxonNomenclature` at parse time would turn every link written before a
/// toggle existed into a search with that toggle off.
#[derive(Clone, Debug, Default, PartialEq)]
struct PartialNomenclature {
    accepted_synonyms: Option<bool>,
    basionyms: Option<bool>,
    protonyms: Option<bool>,
    replacements: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct FormulaQueryDto {
    exact: Option<String>,
    c_min: Option<u16>,
    c_max: Option<u16>,
    h_min: Option<u16>,
    h_max: Option<u16>,
    n_min: Option<u16>,
    n_max: Option<u16>,
    o_min: Option<u16>,
    o_max: Option<u16>,
    p_min: Option<u16>,
    p_max: Option<u16>,
    s_min: Option<u16>,
    s_max: Option<u16>,
    f_state: Option<ElementState>,
    cl_state: Option<ElementState>,
    br_state: Option<ElementState>,
    i_state: Option<ElementState>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct RangeF64Dto {
    min: Option<f64>,
    max: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct RangeU16Dto {
    min: Option<u16>,
    max: Option<u16>,
}
