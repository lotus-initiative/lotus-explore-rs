// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Mapping CLI flags into [`lotus_model::SearchCriteria`].

use crate::args::{Range, SearchArgs};

/// The criteria the flags describe.
///
/// Split out of `search` so it is testable without a network, a terminal or a
/// running binary. Not a cosmetic extraction: the four `--no-*` nomenclatural
/// flags are negations, and nothing downstream can tell "not passed" from
/// "passed, meaning the opposite" unless this is the thing under test. With the
/// mapping inlined, a dropped `!` turned `--no-basionyms` into a way of
/// *enabling* the basionym search and every test still passed.
pub fn criteria_from_args(args: &SearchArgs, clock_year: u16) -> lotus_model::SearchCriteria {
    let mut criteria = lotus_model::SearchCriteria::up_to_year(clock_year);

    criteria.taxon.clone_from(&args.taxon);
    // The model defaults all four to on and each flag is a negation, so these
    // are unconditional assignments rather than defaults the flags turn off.
    criteria.taxon_names = lotus_model::TaxonNomenclature {
        accepted_synonyms: !args.no_accepted_synonyms,
        basionyms: !args.no_basionyms,
        protonyms: !args.no_protonyms,
        replacements: !args.no_replacements,
    };
    criteria.structure.clone_from(&args.structure);
    criteria.structure_search = args.structure_search.into();
    criteria.structure_threshold = args.threshold;
    criteria.mass_min = args.mass_min;
    criteria.mass_max = args.mass_max;
    criteria.reference = args.reference.clone().unwrap_or_default();
    criteria.year_min = args.year_min;
    criteria.year_max = args.year_max.unwrap_or(clock_year);
    criteria.formula_exact = args.formula.clone().unwrap_or_default();
    apply_ranges(
        &mut criteria,
        args.carbon,
        args.hydrogen,
        args.nitrogen,
        args.oxygen,
        args.phosphorus,
        args.sulfur,
    );
    criteria.f_state = args.fluorine.into();
    criteria.cl_state = args.chlorine.into();
    criteria.br_state = args.bromine.into();
    criteria.i_state = args.iodine.into();

    criteria.formula_enabled = flags_ask_for_formula(&criteria);

    criteria
}

pub fn apply_ranges(
    criteria: &mut lotus_model::SearchCriteria,
    carbon: Option<Range>,
    hydrogen: Option<Range>,
    nitrogen: Option<Range>,
    oxygen: Option<Range>,
    phosphorus: Option<Range>,
    sulfur: Option<Range>,
) {
    let set = |range: Option<Range>, min: &mut u16, max: &mut u16, default_max: u16| {
        if let Some(range) = range {
            *min = range.min;
            *max = range.max.unwrap_or(default_max);
        }
    };
    set(
        carbon,
        &mut criteria.c_min,
        &mut criteria.c_max,
        lotus_model::element_max::C,
    );
    set(
        hydrogen,
        &mut criteria.h_min,
        &mut criteria.h_max,
        lotus_model::element_max::H,
    );
    set(
        nitrogen,
        &mut criteria.n_min,
        &mut criteria.n_max,
        lotus_model::element_max::N,
    );
    set(
        oxygen,
        &mut criteria.o_min,
        &mut criteria.o_max,
        lotus_model::element_max::O,
    );
    set(
        phosphorus,
        &mut criteria.p_min,
        &mut criteria.p_max,
        lotus_model::element_max::P,
    );
    set(
        sulfur,
        &mut criteria.s_min,
        &mut criteria.s_max,
        lotus_model::element_max::S,
    );
}

/// Whether any formula/element CLI flag asked for formula filtering.
///
/// `--formula` and `--carbon` both filter by formula, and only the first sets
/// `formula_enabled` on the criteria. This is not
/// [`lotus_model::SearchCriteria::has_formula_filter`], which reports false
/// until `formula_enabled` is set -- asking it here would be circular.
#[must_use]
pub fn flags_ask_for_formula(criteria: &lotus_model::SearchCriteria) -> bool {
    use lotus_model::ElementState;
    !criteria.formula_exact.trim().is_empty()
        || criteria
            .element_ranges()
            .iter()
            .any(|(_, min, max, default_max)| *min > 0 || *max < *default_max)
        || criteria.f_state != ElementState::Allowed
        || criteria.cl_state != ElementState::Allowed
        || criteria.br_state != ElementState::Allowed
        || criteria.i_state != ElementState::Allowed
}
