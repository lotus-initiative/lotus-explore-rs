// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::types::{ValidationCode, ValidationError, ValidationField, ValidationResult};
use lotus_model::SmilesSearchType;

const MAX_TAXON_LEN: usize = 500;
const MAX_STRUCTURE_LEN: usize = 10_000;
const MIN_YEAR: u16 = 1600;
const MAX_MASS: f64 = 10_000.0;
const MAX_ELEMENT_COUNT: u16 = 1000;

/// Validate that a taxon string is not empty and within reasonable bounds.
pub(super) fn validate_taxon(input: &str) -> ValidationResult<()> {
    validate_optional_text_length(
        input,
        MAX_TAXON_LEN,
        ValidationField::Taxon,
        ValidationCode::TaxonTooLong,
    )
}

/// Validate that a SMILES/Molfile string is not too long.
pub(super) fn validate_smiles(input: &str) -> ValidationResult<()> {
    validate_optional_text_length(
        input,
        MAX_STRUCTURE_LEN,
        ValidationField::Smiles,
        ValidationCode::StructureTooLong,
    )?;

    let trimmed = input.trim();
    if trimmed.len() == 1
        && matches!(
            lotus_model::classify_structure(trimmed),
            lotus_model::StructureKind::Smiles
        )
        && trimmed
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase())
    {
        return Err(ValidationError::new(
            ValidationField::Smiles,
            ValidationCode::InvalidStructure,
        ));
    }

    Ok(())
}

/// Validate that a mass value is within valid range.
pub(super) fn validate_mass(value: f64, min: f64, max: f64) -> ValidationResult<()> {
    if !(0.0..=MAX_MASS).contains(&value) {
        return Err(ValidationError::new(
            ValidationField::Mass,
            ValidationCode::MassOutOfRange,
        ));
    }

    if min > max {
        return Err(ValidationError::new(
            ValidationField::Mass,
            ValidationCode::MinGreaterThanMax,
        ));
    }

    Ok(())
}

/// Validate mass range. Helper for validators and test code.
#[cfg(test)]
pub(super) fn validate_mass_range(min: f64, max: f64) -> ValidationResult<()> {
    validate_mass(min, min, max)?;
    validate_mass(max, min, max)
}

/// Validate that a year value is within reasonable range.
pub(super) fn validate_year(value: u16) -> ValidationResult<()> {
    let current_year = crate::clock::current_year();
    if !(MIN_YEAR..=current_year).contains(&value) {
        return Err(ValidationError::new(
            ValidationField::Year,
            ValidationCode::YearOutOfRange,
        ));
    }
    Ok(())
}

/// Validate that a year range is sensible.
pub(super) fn validate_year_range(min: u16, max: u16) -> ValidationResult<()> {
    validate_year(min)?;
    validate_year(max)?;

    if min > max {
        return Err(ValidationError::new(
            ValidationField::Year,
            ValidationCode::YearRangeInvalid,
        ));
    }

    Ok(())
}

/// Validate that an element count is within valid range.
pub(super) fn validate_element_count(value: u16) -> ValidationResult<()> {
    if value > MAX_ELEMENT_COUNT {
        return Err(ValidationError::new(
            ValidationField::Formula,
            ValidationCode::ElementCountHighWarning,
        ));
    }
    Ok(())
}

/// Validate the similarity threshold when similarity search is active.
pub(super) fn validate_similarity_threshold(
    search_type: SmilesSearchType,
    threshold: f64,
) -> ValidationResult<()> {
    if search_type == SmilesSearchType::Similarity && threshold <= 0.0 {
        return Err(ValidationError::new(
            ValidationField::SimilarityThreshold,
            ValidationCode::SimilarityThresholdInvalid,
        ));
    }
    Ok(())
}

fn validate_optional_text_length(
    input: &str,
    max_len: usize,
    field: ValidationField,
    code: ValidationCode,
) -> ValidationResult<()> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    if trimmed.len() > max_len {
        return Err(ValidationError::new(field, code));
    }
    Ok(())
}
