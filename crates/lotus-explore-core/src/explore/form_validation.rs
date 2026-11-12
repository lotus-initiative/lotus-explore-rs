// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Criteria validation at the Explore orchestration boundary.
//!
//! Delegates to [`lotus_model::validate_criteria`] so the web explorer and the
//! CLI enforce one set of rules, translating [`lotus_model::ValidationError`]
//! into [`ValidationFault`].

use crate::explore::types::ValidationFault;
use lotus_model::{SearchCriteria, ValidationError, validate_criteria};

/// Validate criteria against `year_max` before dispatching a search.
///
/// `year_max` is passed in from the host clock so `lotus-explore-core` stays
/// free of wall-clock reads.
///
/// # Errors
///
/// Returns the [`ValidationFault`] corresponding to the first
/// [`lotus_model::ValidationError`] reported by [`lotus_model::validate_criteria`].
pub fn validate_dispatch_criteria(
    criteria: &SearchCriteria,
    year_max: u16,
) -> Result<(), ValidationFault> {
    validate_criteria(criteria, year_max).map_err(|err| match err {
        ValidationError::TaxonTooLong { .. } => ValidationFault::TaxonTooLong,
        ValidationError::StructureTooLong { .. } => ValidationFault::StructureTooLong,
        ValidationError::StructureNotAMolecule => ValidationFault::EmptyInput,
        ValidationError::MassOutOfRange { .. } => ValidationFault::MassOutOfRange,
        ValidationError::MassRangeInverted => ValidationFault::MassRangeInvalid,
        ValidationError::YearOutOfRange { .. } => ValidationFault::YearOutOfRange,
        ValidationError::YearRangeInverted => ValidationFault::YearRangeInvalid,
        ValidationError::ElementCountTooHigh { .. } => ValidationFault::ElementCountTooHigh,
        ValidationError::ElementRangeInverted { .. } => ValidationFault::ElementRangeInvalid,
        ValidationError::SimilarityThresholdInvalid => ValidationFault::SimilarityThresholdInvalid,
        ValidationError::ReferenceNotAIdentifier => ValidationFault::ReferenceNotAnIdentifier {
            input: criteria.reference.trim().to_string(),
        },
    })
}

#[cfg(test)]
#[path = "form_validation/tests.rs"]
mod tests;
