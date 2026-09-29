// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::rules::{
    validate_element_count, validate_mass, validate_similarity_threshold, validate_smiles,
    validate_taxon, validate_year_range,
};
use super::types::ValidationError;
use crate::features::explore::types::ValidationFault;
use crate::models::SearchCriteria;

/// Validate criteria at the orchestration boundary.
/// This validator returns domain-native `ValidationFault` so `start_search`
/// can fail fast without translating from UI-oriented validation error keys.
pub fn validate_dispatch_criteria(criteria: &SearchCriteria) -> Result<(), ValidationFault> {
    if primary_filters_empty(criteria) {
        return Err(ValidationFault::EmptyInput);
    }

    // Safety: map_err is only called when validate_criteria returns Err,
    // which means errors is guaranteed non-empty.
    #[allow(clippy::expect_used)]
    validate_criteria(criteria)
        .map_err(|errors| {
            errors
                .into_iter()
                .next()
                .expect("validation_errors contains at least one error")
        })
        .map_err(ValidationError::into_fault)
}

fn validate_criteria(criteria: &SearchCriteria) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::with_capacity(6);

    push_error(&mut errors, validate_taxon(&criteria.taxon));
    push_error(&mut errors, validate_smiles(&criteria.smiles));
    push_error(
        &mut errors,
        validate_mass(criteria.mass_min, criteria.mass_min, criteria.mass_max),
    );
    push_error(
        &mut errors,
        validate_year_range(criteria.year_min, criteria.year_max),
    );
    push_error(
        &mut errors,
        validate_element_count(criteria.c_min + criteria.h_min),
    );
    push_error(
        &mut errors,
        validate_similarity_threshold(criteria.smiles_search_type, criteria.smiles_threshold),
    );

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn primary_filters_empty(criteria: &SearchCriteria) -> bool {
    criteria.taxon.trim().is_empty()
        && criteria.smiles.trim().is_empty()
        && !criteria.formula_enabled
}

fn push_error(errors: &mut Vec<ValidationError>, result: Result<(), ValidationError>) {
    if let Err(error) = result {
        errors.push(error);
    }
}
