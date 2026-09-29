// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Filter validation.
//!
//! Validation lives here rather than in the UI or the CLI so that a bad filter
//! is rejected the same way on every surface, and so the rules are testable
//! without a browser or a process.

use super::criteria::SearchCriteria;
use super::{MASS_MAX, STRUCTURE_MAX_LEN, TAXON_MAX_LEN, YEAR_MIN};
use thiserror::Error;

/// A filter that cannot be turned into a query.
///
/// The `#[error]` attribute on each variant is its user-facing message, so a doc
/// comment on the variant would only restate it.
#[derive(Debug, Clone, PartialEq, Error)]
#[allow(missing_docs)]
pub enum ValidationError {
    #[error("taxon is longer than {limit} characters")]
    TaxonTooLong { limit: usize },

    #[error("structure is longer than {limit} characters")]
    StructureTooLong { limit: usize },

    #[error("a one-character lowercase structure is not a molecule")]
    StructureNotAMolecule,

    #[error("mass is outside 0..={max}")]
    MassOutOfRange { max: f64 },

    #[error("mass minimum is above the maximum")]
    MassRangeInverted,

    #[error("year is outside {min}..={max}")]
    YearOutOfRange { min: u16, max: u16 },

    #[error("year minimum is above the maximum")]
    YearRangeInverted,

    #[error("{element} count is above {max}, the widest a molecule can plausibly be")]
    ElementCountTooHigh { element: &'static str, max: u16 },

    #[error("similarity search needs a threshold above 0")]
    SimilarityThresholdInvalid,
}

/// Whether a criteria can be turned into a query, given the year it is bounded
/// against.
///
/// `year_max` is passed rather than read from a clock, so that the same criteria
/// validates identically on every surface and in every test.
///
/// # Errors
/// Returns the first [`ValidationError`] the criteria trips, in a fixed order, so
/// that a user is shown one fault at a time rather than a list.
pub fn validate_criteria(criteria: &SearchCriteria, year_max: u16) -> Result<(), ValidationError> {
    if criteria.taxon.trim().len() > TAXON_MAX_LEN {
        return Err(ValidationError::TaxonTooLong {
            limit: TAXON_MAX_LEN,
        });
    }

    validate_structure(&criteria.structure)?;

    if !(0.0..=MASS_MAX).contains(&criteria.mass_min)
        || !(0.0..=MASS_MAX).contains(&criteria.mass_max)
    {
        return Err(ValidationError::MassOutOfRange { max: MASS_MAX });
    }
    if criteria.mass_min > criteria.mass_max {
        return Err(ValidationError::MassRangeInverted);
    }

    for year in [criteria.year_min, criteria.year_max] {
        if !(YEAR_MIN..=year_max).contains(&year) {
            return Err(ValidationError::YearOutOfRange {
                min: YEAR_MIN,
                max: year_max,
            });
        }
    }
    if criteria.year_min > criteria.year_max {
        return Err(ValidationError::YearRangeInverted);
    }

    // Each element's ceiling is its own default maximum, not one global number:
    // hydrogen's default is 1024 and carbon's is 512, so a shared limit of 1000
    // would reject the hydrogen default outright. A bound above the default is
    // a filter that can only widen the result set, so it is rejected.
    for (symbol, min, max, default_max) in criteria.element_ranges() {
        for count in [min, max] {
            if count > default_max {
                return Err(ValidationError::ElementCountTooHigh {
                    element: symbol,
                    max: default_max,
                });
            }
        }
    }

    if criteria.structure_search == super::SmilesSearchType::Similarity
        && criteria.structure_threshold <= 0.0
    {
        return Err(ValidationError::SimilarityThresholdInvalid);
    }

    Ok(())
}

fn validate_structure(structure: &str) -> Result<(), ValidationError> {
    let trimmed = structure.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    if trimmed.len() > STRUCTURE_MAX_LEN {
        return Err(ValidationError::StructureTooLong {
            limit: STRUCTURE_MAX_LEN,
        });
    }
    // A single lowercase letter is a typo — a dropped `1` in `C1CC1`, a stray `c`
    // — and the endpoint would answer with an unhelpful parse error.
    if trimmed.chars().count() == 1
        && super::classify_structure(trimmed) == super::StructureKind::Smiles
        && trimmed.starts_with(|c: char| c.is_ascii_lowercase())
    {
        return Err(ValidationError::StructureNotAMolecule);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
}
