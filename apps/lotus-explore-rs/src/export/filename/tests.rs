// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `filename`, in their own file.

use super::*;
use lotus_model::SmilesSearchType;

#[test]
fn export_filename_taxon_only_has_no_filtered_suffix() {
    let criteria = SearchCriteria {
        taxon: "Gentiana lutea".into(),
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };
    let name = generate_filename(&criteria, "csv");
    assert!(!name.contains("_filtered."));
    assert!(name.ends_with("_Gentiana_lutea.csv"));
}

#[test]
fn export_filename_for_full_dataset_has_no_filtered_suffix() {
    let criteria = SearchCriteria {
        taxon: "*".into(),
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };
    let name = generate_filename(&criteria, "csv");
    assert!(!name.contains("_filtered."));
    assert!(name.ends_with("_all_taxa.csv"));
}

#[test]
fn export_filename_with_structure_filter_keeps_search_type() {
    let mut criteria = SearchCriteria {
        taxon: "*".into(),
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };
    criteria.structure = "c1ccccc1".into();
    criteria.structure_search = SmilesSearchType::Similarity;
    let name = generate_filename(&criteria, "rdf");
    assert!(name.ends_with("_all_taxa_similarity_filtered.rdf"));
}
