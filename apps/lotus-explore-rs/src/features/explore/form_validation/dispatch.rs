// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::rules::{
    validate_element_count, validate_mass, validate_similarity_threshold, validate_smiles,
    validate_taxon, validate_year_range,
};
use super::types::ValidationError;
use crate::features::explore::types::ValidationFault;
use lotus_search::SearchCriteria;

/// Validate criteria at the orchestration boundary.
/// This validator returns domain-native `ValidationFault` so `start_search`
/// can fail fast without translating from UI-oriented validation error keys.
pub fn validate_dispatch_criteria(criteria: &SearchCriteria) -> Result<(), ValidationFault> {
    if primary_filters_empty(criteria) {
        return Err(ValidationFault::EmptyInput);
    }

    // `validation_errors` only ever fails with at least one error, because each
    // rule is added by `push_error`, which takes an `Option`. That was a comment
    // and an `expect`; it is now a `map_or` whose fallback arm names the same
    // invariant and returns a fault instead of panicking, so a rule that one day
    // fails with nothing to report produces a wrong message rather than a crash.
    match validation_errors(criteria) {
        Ok(()) => Ok(()),
        Err(errors) => Err(errors
            .first()
            .map_or(ValidationFault::EmptyInput, ValidationError::as_fault)),
    }
}

/// Every rule that rejects this criteria, in the order a reader would fix them.
///
/// Named for what it returns rather than for what it does: the public
///  stops at the first error, and a form wants
/// all of them so a reader can fix the fields in one pass.
fn validation_errors(criteria: &SearchCriteria) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::with_capacity(6);

    push_error(&mut errors, validate_taxon(&criteria.taxon));
    push_error(&mut errors, validate_smiles(&criteria.structure));
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
        validate_similarity_threshold(criteria.structure_search, criteria.structure_threshold),
    );

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn primary_filters_empty(criteria: &SearchCriteria) -> bool {
    criteria.taxon.trim().is_empty()
        && criteria.structure.trim().is_empty()
        && !criteria.formula_enabled
}

fn push_error(errors: &mut Vec<ValidationError>, result: Result<(), ValidationError>) {
    if let Err(error) = result {
        errors.push(error);
    }
}
