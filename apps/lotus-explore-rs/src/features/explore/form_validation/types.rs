// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::features::explore::types::ValidationFault;

pub(super) type ValidationResult<T> = Result<T, ValidationError>;

/// Strongly-typed validation code used for fault mapping and UI error routing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ValidationCode {
    TaxonTooLong,
    StructureTooLong,
    InvalidStructure,
    MassOutOfRange,
    MinGreaterThanMax,
    YearOutOfRange,
    YearRangeInvalid,
    ElementCountHighWarning,
    SimilarityThresholdInvalid,
}

/// Strongly-typed field identifier for validation targeting in UI forms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ValidationField {
    Taxon,
    Smiles,
    Mass,
    Year,
    Formula,
    SimilarityThreshold,
}

/// A validation error with localization context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ValidationError {
    /// Field name for error targeting.
    pub field: ValidationField,
    /// Typed validation code (stable contract for domain fault mapping).
    pub code: ValidationCode,
}

impl ValidationError {
    pub(super) const fn new(field: ValidationField, code: ValidationCode) -> Self {
        Self { field, code }
    }

    /// The domain fault this error maps to.
    ///
    /// `as_` rather than `into_` because it borrows: the caller has a `&`
    /// from `Vec::first`, and taking `self` would mean cloning the error to
    /// throw it away.
    pub(super) const fn as_fault(&self) -> ValidationFault {
        match self.code {
            ValidationCode::TaxonTooLong => ValidationFault::TaxonTooLong,
            ValidationCode::StructureTooLong => ValidationFault::StructureTooLong,
            ValidationCode::InvalidStructure => ValidationFault::EmptyInput,
            ValidationCode::MassOutOfRange => ValidationFault::MassOutOfRange,
            ValidationCode::MinGreaterThanMax => ValidationFault::MassRangeInvalid,
            ValidationCode::YearOutOfRange => ValidationFault::YearOutOfRange,
            ValidationCode::YearRangeInvalid => ValidationFault::YearRangeInvalid,
            ValidationCode::ElementCountHighWarning => ValidationFault::ElementCountTooHigh,
            ValidationCode::SimilarityThresholdInvalid => {
                ValidationFault::SimilarityThresholdInvalid
            }
        }
    }
}
