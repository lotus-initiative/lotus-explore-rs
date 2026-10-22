// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `filename`, in their own file.

use super::*;
use lotus_model::SmilesSearchType;

/// The year the tests pin, so "is this year bound in the future?" has one
/// answer rather than the answer the day the suite runs.
const YEAR: u16 = 2026;

/// The instant the tests pin, so the name they assert is the whole name.
const NOW: &str = "2026-03-04T05:06:07Z";

#[test]
fn export_filename_taxon_only_has_no_filtered_suffix() {
    let criteria = SearchCriteria {
        taxon: "Gentiana lutea".into(),
        ..SearchCriteria::up_to_year(YEAR)
    };
    let name = generate_filename(&criteria, "csv", YEAR, NOW);
    assert!(!name.contains("_filtered."));
    assert!(name.ends_with("_Gentiana_lutea.csv"));
}

#[test]
fn export_filename_for_full_dataset_has_no_filtered_suffix() {
    let criteria = SearchCriteria {
        taxon: "*".into(),
        ..SearchCriteria::up_to_year(YEAR)
    };
    let name = generate_filename(&criteria, "csv", YEAR, NOW);
    assert!(!name.contains("_filtered."));
    assert!(name.ends_with("_all_taxa.csv"));
}

#[test]
fn export_filename_with_structure_filter_keeps_search_type() {
    let mut criteria = SearchCriteria {
        taxon: "*".into(),
        ..SearchCriteria::up_to_year(YEAR)
    };
    criteria.structure = "c1ccccc1".into();
    criteria.structure_search = SmilesSearchType::Similarity;
    let name = generate_filename(&criteria, "ttl", YEAR, NOW);
    assert!(name.ends_with("_all_taxa_similarity_filtered.ttl"));
}

#[test]
fn export_filename_starts_with_the_date_taken_from_the_stamp() {
    let criteria = SearchCriteria {
        taxon: "*".into(),
        ..SearchCriteria::up_to_year(YEAR)
    };
    let name = generate_filename(&criteria, "csv", YEAR, NOW);
    assert!(name.starts_with("20260304_lotus_"), "{name}");
}

#[test]
fn today_yyyymmdd_takes_the_stamps_first_eight_digits() {
    assert_eq!(today_yyyymmdd(NOW), "20260304");
}
