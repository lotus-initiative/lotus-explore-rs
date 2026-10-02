// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

#![allow(clippy::manual_string_new)]
#![allow(clippy::expect_used)]

use super::dispatch::validate_dispatch_criteria;
use super::rules::{
    validate_element_count, validate_mass, validate_mass_range, validate_smiles, validate_taxon,
    validate_year_range,
};
use super::types::{ValidationCode, ValidationField};
use crate::features::explore::types::ValidationFault;
use lotus_model::{SearchCriteria, SmilesSearchType};

#[test]
fn validate_taxon_accepts_empty_string() {
    assert!(validate_taxon("").is_ok());
}

#[test]
fn validate_taxon_accepts_valid_input() {
    assert!(validate_taxon("Rosa").is_ok());
}

#[test]
fn validate_taxon_rejects_extremely_long_input() {
    let long = "x".repeat(501);
    assert!(validate_taxon(&long).is_err());
}

#[test]
fn validate_smiles_accepts_empty_string() {
    assert!(validate_smiles("").is_ok());
}

#[test]
fn validate_smiles_accepts_valid_smiles() {
    assert!(validate_smiles("CC(C)Cc1ccc(cc1)C(C)C(=O)O").is_ok());
}

#[test]
fn validate_smiles_rejects_extremely_long_input() {
    let long = "C".repeat(10_001);
    assert!(validate_smiles(&long).is_err());
}

#[test]
fn validate_smiles_rejects_single_lowercase_token() {
    assert!(validate_smiles("d").is_err());
}

#[test]
fn validate_mass_range_rejects_inverted_range() {
    assert!(validate_mass_range(200.0, 100.0).is_err());
}

#[test]
fn validate_mass_range_accepts_valid_range() {
    assert!(validate_mass_range(100.0, 200.0).is_ok());
}

#[test]
fn validate_year_range_rejects_inverted_range() {
    let result = validate_year_range(2025, 2020);
    assert!(result.is_err());
}

#[test]
fn validate_year_range_accepts_valid_range() {
    let result = validate_year_range(2000, 2025);
    assert!(result.is_ok());
}

#[test]
fn validate_element_count_accepts_reasonable_counts() {
    assert!(validate_element_count(100).is_ok());
}

#[test]
fn validate_element_count_rejects_unreasonably_high_counts() {
    assert!(validate_element_count(1001).is_err());
}

#[test]
fn validate_dispatch_criteria_accepts_a_search_that_names_nothing() {
    // Refusing this was a kindness that cost the reader their question. "Everything
    // published in 2019" is a real one, and it needs no structure and no taxon --
    // so the answer is a notice saying what it covers, not an error.
    let criteria = SearchCriteria {
        taxon: "   ".into(),
        structure: "".into(),
        formula_enabled: false,
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };
    assert_eq!(validate_dispatch_criteria(&criteria), Ok(()));
}

#[test]
fn a_search_is_unconstrained_only_when_it_names_nothing_at_all() {
    use crate::features::explore::form_validation::is_unconstrained;
    let year = crate::clock::current_year();

    // Blank in all three is the case the notice exists for: nothing narrows the
    // scan, so the whole database is walked.
    assert!(is_unconstrained(&SearchCriteria {
        taxon: "  ".into(),
        structure: "".into(),
        reference: "  ".into(),
        ..SearchCriteria::up_to_year(year)
    }));

    // A reference names one paper, and one paper's compounds is a small, ordinary
    // answer. Calling that "every compound LOTUS holds" would be wrong in exactly
    // the case where the reader already knows what they asked for.
    assert!(!is_unconstrained(&SearchCriteria {
        taxon: "".into(),
        structure: "".into(),
        reference: "10.1002/andp.18280880206".into(),
        ..SearchCriteria::up_to_year(year)
    }));

    // Mass and year are filters on top of the same scan, not a narrower subject,
    // so they do not silence the notice.
    assert!(is_unconstrained(&SearchCriteria {
        taxon: "".into(),
        structure: "".into(),
        mass_min: 100.0,
        year_min: 2019,
        ..SearchCriteria::up_to_year(year)
    }));

    assert!(!is_unconstrained(&SearchCriteria {
        taxon: "Gentiana lutea".into(),
        structure: "".into(),
        ..SearchCriteria::up_to_year(year)
    }));
    assert!(!is_unconstrained(&SearchCriteria {
        taxon: "".into(),
        structure: "CCO".into(),
        ..SearchCriteria::up_to_year(year)
    }));
}

#[test]
fn a_formula_only_search_is_still_a_whole_database_scan() {
    // A formula narrows which rows pass, not which compounds are searched for, so
    // the walk underneath it is the same one. That is the whole point of wording
    // the notice as a fact about the scan: it stays true here.
    use crate::features::explore::form_validation::is_unconstrained;
    let criteria = SearchCriteria {
        taxon: "".into(),
        structure: "".into(),
        formula_enabled: true,
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };
    assert!(is_unconstrained(&criteria));
    assert_eq!(validate_dispatch_criteria(&criteria), Ok(()));
}

#[test]
fn validate_dispatch_criteria_accepts_formula_only_search() {
    let criteria = SearchCriteria {
        taxon: "".into(),
        structure: "".into(),
        formula_enabled: true,
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };
    assert_eq!(validate_dispatch_criteria(&criteria), Ok(()));
}

#[test]
fn validate_dispatch_criteria_maps_mass_out_of_range_to_domain_fault() {
    let criteria = SearchCriteria {
        taxon: "Rosa".into(),
        mass_min: -1.0,
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };

    assert_eq!(
        validate_dispatch_criteria(&criteria),
        Err(ValidationFault::MassOutOfRange)
    );
}

#[test]
fn validate_dispatch_criteria_maps_year_range_to_domain_fault() {
    let criteria = SearchCriteria {
        taxon: "Rosa".into(),
        year_min: 2025,
        year_max: 2020,
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };

    assert_eq!(
        validate_dispatch_criteria(&criteria),
        Err(ValidationFault::YearRangeInvalid)
    );
}

#[test]
fn validation_error_uses_typed_field_for_taxon() {
    let long = "x".repeat(501);
    let err = validate_taxon(&long).expect_err("expected taxon length error");
    assert_eq!(err.field, ValidationField::Taxon);
    assert_eq!(err.code, ValidationCode::TaxonTooLong);
}

#[test]
fn validation_error_uses_typed_field_for_mass() {
    let err = validate_mass(-1.0, -1.0, 100.0).expect_err("expected mass range error");
    assert_eq!(err.field, ValidationField::Mass);
    assert_eq!(err.code, ValidationCode::MassOutOfRange);
}

#[test]
fn validate_dispatch_criteria_rejects_zero_similarity_threshold() {
    let criteria = SearchCriteria {
        taxon: "Rosa".into(),
        structure: "c1ccccc1".into(),
        structure_search: SmilesSearchType::Similarity,
        structure_threshold: 0.0,
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };
    assert_eq!(
        validate_dispatch_criteria(&criteria),
        Err(ValidationFault::SimilarityThresholdInvalid)
    );
}

#[test]
fn validate_dispatch_criteria_accepts_positive_similarity_threshold() {
    let criteria = SearchCriteria {
        taxon: "Rosa".into(),
        structure: "c1ccccc1".into(),
        structure_search: SmilesSearchType::Similarity,
        structure_threshold: 0.7,
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };
    assert_eq!(validate_dispatch_criteria(&criteria), Ok(()));
}

#[test]
fn validate_dispatch_criteria_ignores_threshold_for_substructure() {
    let criteria = SearchCriteria {
        taxon: "Rosa".into(),
        structure: "c1ccccc1".into(),
        structure_search: SmilesSearchType::Substructure,
        structure_threshold: 0.0,
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };
    assert_eq!(validate_dispatch_criteria(&criteria), Ok(()));
}

#[test]
fn validate_dispatch_criteria_rejects_malformed_single_letter_structure() {
    let criteria = SearchCriteria {
        taxon: "Rosa".into(),
        structure: "d".into(),
        structure_search: SmilesSearchType::Substructure,
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };
    assert_eq!(
        validate_dispatch_criteria(&criteria),
        Err(ValidationFault::EmptyInput)
    );
}
