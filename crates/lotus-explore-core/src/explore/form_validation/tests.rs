// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::validate_dispatch_criteria;
use crate::explore::types::ValidationFault;
use lotus_model::{SearchCriteria, SmilesSearchType};

const YEAR: u16 = 2027;

#[test]
fn accepts_unconstrained_and_formula_only_searches() {
    let unconstrained = SearchCriteria {
        taxon: "   ".into(),
        ..SearchCriteria::up_to_year(YEAR)
    };
    assert_eq!(validate_dispatch_criteria(&unconstrained, YEAR), Ok(()));

    let formula_only = SearchCriteria {
        formula_enabled: true,
        ..SearchCriteria::up_to_year(YEAR)
    };
    assert_eq!(validate_dispatch_criteria(&formula_only, YEAR), Ok(()));
}

#[test]
fn maps_taxon_and_structure_faults() {
    let long_taxon = SearchCriteria {
        taxon: "x".repeat(501),
        ..SearchCriteria::up_to_year(YEAR)
    };
    assert_eq!(
        validate_dispatch_criteria(&long_taxon, YEAR),
        Err(ValidationFault::TaxonTooLong)
    );

    let long_structure = SearchCriteria {
        structure: "C".repeat(10_001),
        ..SearchCriteria::up_to_year(YEAR)
    };
    assert_eq!(
        validate_dispatch_criteria(&long_structure, YEAR),
        Err(ValidationFault::StructureTooLong)
    );

    let single_lowercase = SearchCriteria {
        structure: "d".into(),
        structure_search: SmilesSearchType::Substructure,
        ..SearchCriteria::up_to_year(YEAR)
    };
    assert_eq!(
        validate_dispatch_criteria(&single_lowercase, YEAR),
        Err(ValidationFault::EmptyInput)
    );
}

#[test]
fn maps_reference_mass_and_year_faults() {
    let bad_ref = SearchCriteria {
        reference: "  not-an-id  ".into(),
        ..SearchCriteria::up_to_year(YEAR)
    };
    assert_eq!(
        validate_dispatch_criteria(&bad_ref, YEAR),
        Err(ValidationFault::ReferenceNotAnIdentifier {
            input: "not-an-id".into()
        })
    );

    let mass_out_of_range = SearchCriteria {
        mass_min: -1.0,
        ..SearchCriteria::up_to_year(YEAR)
    };
    assert_eq!(
        validate_dispatch_criteria(&mass_out_of_range, YEAR),
        Err(ValidationFault::MassOutOfRange)
    );

    let mass_inverted = SearchCriteria {
        mass_min: 200.0,
        mass_max: 100.0,
        ..SearchCriteria::up_to_year(YEAR)
    };
    assert_eq!(
        validate_dispatch_criteria(&mass_inverted, YEAR),
        Err(ValidationFault::MassRangeInvalid)
    );

    let year_out_of_range = SearchCriteria {
        year_max: YEAR + 1,
        ..SearchCriteria::up_to_year(YEAR)
    };
    assert_eq!(
        validate_dispatch_criteria(&year_out_of_range, YEAR),
        Err(ValidationFault::YearOutOfRange)
    );

    let year_inverted = SearchCriteria {
        year_min: 2025,
        year_max: 2020,
        ..SearchCriteria::up_to_year(YEAR)
    };
    assert_eq!(
        validate_dispatch_criteria(&year_inverted, YEAR),
        Err(ValidationFault::YearRangeInvalid)
    );
}

#[test]
fn maps_per_element_ceilings_inverted_ranges_and_similarity_threshold() {
    // Hydrogen's default ceiling is 1024; the old sum-of-1000 duplicate rule
    // rejected `c_min=500, h_min=600` even though both lie within model bounds.
    let valid_high_hydrogen = SearchCriteria {
        c_min: 500,
        h_min: 600,
        ..SearchCriteria::up_to_year(YEAR)
    };
    assert_eq!(
        validate_dispatch_criteria(&valid_high_hydrogen, YEAR),
        Ok(())
    );

    let element_too_high = SearchCriteria {
        c_max: 513,
        ..SearchCriteria::up_to_year(YEAR)
    };
    assert_eq!(
        validate_dispatch_criteria(&element_too_high, YEAR),
        Err(ValidationFault::ElementCountTooHigh)
    );

    let element_inverted = SearchCriteria {
        n_min: 10,
        n_max: 5,
        ..SearchCriteria::up_to_year(YEAR)
    };
    assert_eq!(
        validate_dispatch_criteria(&element_inverted, YEAR),
        Err(ValidationFault::ElementRangeInvalid)
    );

    let zero_similarity = SearchCriteria {
        structure: "c1ccccc1".into(),
        structure_search: SmilesSearchType::Similarity,
        structure_threshold: 0.0,
        ..SearchCriteria::up_to_year(YEAR)
    };
    assert_eq!(
        validate_dispatch_criteria(&zero_similarity, YEAR),
        Err(ValidationFault::SimilarityThresholdInvalid)
    );
}
