// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for the filter set and its `has_*` predicates.
//!
//! The predicates decide what is worth a round trip, so each is checked against
//! the defaults it compares to rather than against a literal.

// The panic lints keep library code from panicking on bad input; a test
// failing on a bad value is reporting, not panicking.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use super::*;

fn criteria() -> SearchCriteria {
    SearchCriteria::up_to_year(2026)
}

#[test]
fn a_fresh_criteria_has_no_active_filter() {
    let c = criteria();
    assert!(!c.has_mass_filter());
    assert!(!c.has_year_filter(2026));
    assert!(!c.has_formula_filter());
    assert!(!c.has_effective_filters(2026));
    assert!(
        !c.is_searchable(),
        "no taxon and no structure is not a search"
    );
}

#[test]
fn the_taxon_alone_makes_a_criteria_searchable() {
    let c = SearchCriteria {
        taxon: "Gentiana lutea".into(),
        ..criteria()
    };
    assert!(c.is_searchable());
    assert!(!c.has_effective_filters(2026), "a taxon is not a filter");
}

#[test]
fn a_structure_alone_makes_a_criteria_searchable() {
    let c = SearchCriteria {
        structure: "c1ccccc1".into(),
        ..criteria()
    };
    assert!(c.is_searchable());
    assert!(c.has_effective_filters(2026));
}

#[test]
fn the_year_predicate_compares_against_the_year_it_is_given() {
    // The same criteria is filtered or not depending on the reference year,
    // which is why the reference is a parameter and not a clock read.
    let c = SearchCriteria {
        year_max: 2020,
        ..criteria()
    };
    assert!(c.has_year_filter(2026), "2020 < 2026");
    assert!(!c.has_year_filter(2020), "2020 is not less than itself");

    let narrowed = SearchCriteria {
        year_min: 2000,
        ..criteria()
    };
    assert!(narrowed.has_year_filter(1990), "2000 > 1800 regardless");
}

#[test]
fn the_mass_predicate_ignores_the_default_bounds() {
    let mut c = criteria();
    c.mass_min = 0.0;
    c.mass_max = MASS_MAX;
    assert!(!c.has_mass_filter());
    c.mass_max = 499.9;
    assert!(c.has_mass_filter());
}

#[test]
fn formula_filtering_needs_the_flag_as_well_as_a_value() {
    let mut c = criteria();
    c.formula_exact = "C17H12O7".into();
    assert!(
        !c.has_formula_filter(),
        "a value with the flag off is inert"
    );
    c.formula_enabled = true;
    assert!(c.has_formula_filter());
}

#[test]
fn a_widened_element_bound_counts_as_a_filter() {
    let mut c = criteria();
    c.formula_enabled = true;
    c.c_min = 5;
    assert!(c.has_formula_filter());
    c.c_min = 0;
    c.s_max = element_max::S - 1;
    assert!(c.has_formula_filter(), "narrowing the ceiling also filters");
}

#[test]
fn a_forbidden_halogen_counts_as_a_formula_filter() {
    let mut c = criteria();
    c.formula_enabled = true;
    c.br_state = ElementState::Excluded;
    assert!(c.has_formula_filter());
}

// ── Boundary cases around the "is this a filter?" predicates ─────────────
//
// These are all `min > 0 || max < default` guards. Each half is reachable
// alone, and a mutation that turns one `>` into `<` makes a real filter look
// like no filter at all -- which widens every result set instead of failing.

#[test]
fn a_mass_lower_bound_alone_is_a_filter() {
    let mut c = criteria();
    c.mass_min = 10.0;
    c.mass_max = crate::MASS_MAX;
    assert!(c.has_mass_filter(), "a lower bound with the default max");
}

#[test]
fn a_mass_upper_bound_alone_is_a_filter() {
    let mut c = criteria();
    c.mass_min = 0.0;
    c.mass_max = crate::MASS_MAX - 1.0;
    assert!(c.has_mass_filter(), "an upper bound with no lower");
}

#[test]
fn the_untouched_mass_range_is_not_a_filter() {
    let c = criteria();
    assert!(!c.has_mass_filter(), "0..=MASS_MAX constrains nothing");
}

#[test]
fn an_element_lower_bound_alone_is_a_formula_filter() {
    let mut c = criteria();
    c.formula_enabled = true;
    c.c_min = 5;
    c.c_max = element_max::C;
    assert!(c.has_formula_filter());
}

#[test]
fn an_element_upper_bound_alone_is_a_formula_filter() {
    let mut c = criteria();
    c.formula_enabled = true;
    c.c_min = 0;
    c.c_max = 3;
    assert!(c.has_formula_filter());
}

#[test]
fn elements_at_their_defaults_are_not_a_formula_filter() {
    let mut c = criteria();
    c.formula_enabled = true;
    // Every element left at its full range, and every halogen allowed.
    assert!(
        !c.has_formula_filter(),
        "the flag alone does not make a filter; a value has to differ"
    );
}

#[test]
fn a_required_or_excluded_halogen_is_a_formula_filter() {
    for state in [ElementState::Required, ElementState::Excluded] {
        let mut c = criteria();
        c.formula_enabled = true;
        c.f_state = state;
        assert!(
            c.has_formula_filter(),
            "{state:?} is a constraint, and only Allowed is not"
        );
    }
    let mut c = criteria();
    c.formula_enabled = true;
    c.f_state = ElementState::Allowed;
    assert!(
        !c.has_formula_filter(),
        "Allowed is the absence of a constraint"
    );
}

#[test]
fn each_predicate_alone_makes_the_criteria_effective() {
    // `has_effective_filters` is a chain of `||`, so each link has to be able
    // to carry the whole answer on its own. `&&` in place of any one of them
    // reports a search with real filters as unfiltered.
    let mut by_structure = criteria();
    by_structure.structure = "CCO".into();
    assert!(by_structure.has_effective_filters(crate::YEAR_MIN));

    let mut by_mass = criteria();
    by_mass.mass_min = 10.0;
    assert!(by_mass.has_effective_filters(crate::YEAR_MIN));

    let mut by_year = criteria();
    by_year.year_min = 1990;
    assert!(by_year.has_effective_filters(crate::YEAR_MIN));

    let mut by_formula = criteria();
    by_formula.formula_enabled = true;
    by_formula.formula_exact = "CCO".into();
    assert!(by_formula.has_effective_filters(crate::YEAR_MIN));
}

#[test]
fn a_taxon_on_its_own_is_not_an_effective_filter() {
    // The taxon is what makes a criteria searchable; it does not narrow a
    // result set, so it must not count as a filter.
    let mut c = criteria();
    c.taxon = "Q16521".into();
    assert!(c.is_searchable());
    assert!(!c.has_effective_filters(crate::YEAR_MIN));
}
