// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for the criteria validator.
//!
//! Each case is a rejection: the point of `validate_criteria` is what it refuses,
//! because an inverted range reaching the endpoint is a query that cannot answer.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use super::*;
use crate::SmilesSearchType;

fn criteria() -> SearchCriteria {
    SearchCriteria::up_to_year(2026)
}

#[test]
fn a_fresh_criteria_validates() {
    assert_eq!(validate_criteria(&criteria(), 2026), Ok(()));
}

#[test]
fn a_lone_lowercase_letter_is_rejected_before_the_endpoint_sees_it() {
    let c = SearchCriteria {
        structure: "c".into(),
        ..criteria()
    };
    assert_eq!(
        validate_criteria(&c, 2026),
        Err(ValidationError::StructureNotAMolecule)
    );
    // A single uppercase atom is a real (if small) molecule.
    let ok = SearchCriteria {
        structure: "C".into(),
        ..criteria()
    };
    assert_eq!(validate_criteria(&ok, 2026), Ok(()));
}

#[test]
fn an_inverted_range_is_rejected_rather_than_silently_swapped() {
    let mass = SearchCriteria {
        mass_min: 400.0,
        mass_max: 100.0,
        ..criteria()
    };
    assert_eq!(
        validate_criteria(&mass, 2026),
        Err(ValidationError::MassRangeInverted)
    );

    let year = SearchCriteria {
        year_min: 2010,
        year_max: 2000,
        ..criteria()
    };
    assert_eq!(
        validate_criteria(&year, 2026),
        Err(ValidationError::YearRangeInverted)
    );
}

#[test]
fn a_year_beyond_the_reference_year_is_rejected() {
    let c = SearchCriteria {
        year_max: 2030,
        ..criteria()
    };
    assert_eq!(
        validate_criteria(&c, 2026),
        Err(ValidationError::YearOutOfRange {
            min: 1800,
            max: 2026
        })
    );
}

#[test]
fn an_element_count_above_that_elements_ceiling_is_rejected() {
    // Per-element, not one global limit: hydrogen's default is 1024 and
    // carbon's is 512, so a shared ceiling would reject the default.
    let carbon = SearchCriteria {
        c_min: 2000,
        ..criteria()
    };
    assert_eq!(
        validate_criteria(&carbon, 2026),
        Err(ValidationError::ElementCountTooHigh {
            element: "C",
            max: 512
        })
    );

    let sulfur = SearchCriteria {
        s_max: 200,
        ..criteria()
    };
    assert_eq!(
        validate_criteria(&sulfur, 2026),
        Err(ValidationError::ElementCountTooHigh {
            element: "S",
            max: 64
        })
    );
}

#[test]
fn similarity_needs_a_threshold() {
    let c = SearchCriteria {
        structure_search: SmilesSearchType::Similarity,
        structure_threshold: 0.0,
        ..criteria()
    };
    assert_eq!(
        validate_criteria(&c, 2026),
        Err(ValidationError::SimilarityThresholdInvalid)
    );
}

#[test]
fn the_first_fault_wins_so_the_message_is_stable() {
    // Two problems at once: the taxon is over-long *and* the mass range is
    // inverted. The taxon is reported, so a user fixes one thing at a time.
    let c = SearchCriteria {
        taxon: "x".repeat(TAXON_MAX_LEN + 1),
        mass_min: 400.0,
        mass_max: 100.0,
        ..criteria()
    };
    assert_eq!(
        validate_criteria(&c, 2026),
        Err(ValidationError::TaxonTooLong {
            limit: TAXON_MAX_LEN
        })
    );
}

// ── The boundary each limit is allowed to reach ─────────────────────────
//
// Every one of these is a `>` where `<=` is what the range means. Tightening
// it to `>=` rejects the value the documentation says is valid; loosening it
// to `==` accepts one past the end. Both are off-by-one on a public limit.

#[test]
fn a_taxon_exactly_at_the_limit_is_accepted() {
    let mut c = criteria();
    c.taxon = "Q".repeat(crate::TAXON_MAX_LEN);
    assert!(
        validate_criteria(&c, 2026).is_ok(),
        "the limit itself is valid"
    );
    c.taxon = "Q".repeat(crate::TAXON_MAX_LEN + 1);
    assert!(matches!(
        validate_criteria(&c, 2026),
        Err(ValidationError::TaxonTooLong { .. })
    ));
}

#[test]
fn a_taxon_at_the_limit_is_measured_after_trimming() {
    let mut c = criteria();
    c.taxon = format!("  {}  ", "Q".repeat(crate::TAXON_MAX_LEN));
    assert!(
        validate_criteria(&c, 2026).is_ok(),
        "padding is not part of the taxon, and counting it rejects a valid QID"
    );
}

#[test]
fn either_mass_bound_out_of_range_is_rejected() {
    // The two are checked independently, so a bad maximum is caught even
    // when the minimum is fine, and vice versa.
    let mut low = criteria();
    low.mass_max = crate::MASS_MAX + 1.0;
    assert!(matches!(
        validate_criteria(&low, 2026),
        Err(ValidationError::MassOutOfRange { .. })
    ));
    let mut high = criteria();
    high.mass_min = -1.0;
    assert!(matches!(
        validate_criteria(&high, 2026),
        Err(ValidationError::MassOutOfRange { .. })
    ));
}

#[test]
fn a_mass_range_of_one_value_is_not_inverted() {
    let mut c = criteria();
    c.mass_min = 100.0;
    c.mass_max = 100.0;
    assert!(
        validate_criteria(&c, 2026).is_ok(),
        "an exact mass is the most precise filter there is, not an inverted one"
    );
}

#[test]
fn a_year_range_of_one_value_is_not_inverted() {
    let mut c = criteria();
    c.year_min = 2000;
    c.year_max = 2000;
    assert!(validate_criteria(&c, 2026).is_ok());
}

#[test]
fn a_similarity_search_with_a_threshold_is_valid() {
    let mut c = criteria();
    c.structure = "CCO".into();
    c.structure_search = SmilesSearchType::Similarity;
    c.structure_threshold = 0.0;
    assert!(
        matches!(
            validate_criteria(&c, 2026),
            Err(ValidationError::SimilarityThresholdInvalid)
        ),
        "zero is not a threshold"
    );
    c.structure_threshold = 0.5;
    assert!(validate_criteria(&c, 2026).is_ok());
}

#[test]
fn a_zero_threshold_is_fine_for_the_other_search_types() {
    // The threshold is only consulted for a similarity search. Applying it
    // unconditionally would reject a substructure search that never uses it.
    let mut c = criteria();
    c.structure = "CCO".into();
    c.structure_search = SmilesSearchType::Substructure;
    c.structure_threshold = 0.0;
    assert!(
        validate_criteria(&c, 2026).is_ok(),
        "a substructure search does not use a threshold"
    );
}

#[test]
fn a_structure_exactly_at_the_limit_is_accepted() {
    let mut c = criteria();
    c.structure = "C".repeat(crate::STRUCTURE_MAX_LEN);
    assert!(validate_criteria(&c, 2026).is_ok());
    c.structure = "C".repeat(crate::STRUCTURE_MAX_LEN + 1);
    assert!(matches!(
        validate_criteria(&c, 2026),
        Err(ValidationError::StructureTooLong { .. })
    ));
}

#[test]
fn an_empty_structure_is_valid_however_long_the_whitespace() {
    let mut c = criteria();
    c.structure = "   \t  ".into();
    assert!(validate_criteria(&c, 2026).is_ok(), "nothing was typed");
}
