// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `dto`, in their own file.

#![allow(clippy::expect_used)]

use super::*;

/// The year the tests pin, so "is this year bound in the future?" has one
/// answer rather than the answer the day the suite runs.
const YEAR: u16 = 2026;

#[test]
fn request_builder_keeps_large_formula_ranges() {
    let mut criteria = SearchCriteria {
        taxon: "*".into(),
        ..SearchCriteria::up_to_year(YEAR)
    };
    criteria.formula_enabled = true;
    criteria.c_max = 300;
    criteria.h_max = 900;

    let request = SearchRequest::from_criteria(&criteria, 123, true, YEAR);
    assert_eq!(request.c_max, Some(300));
    assert_eq!(request.h_max, Some(900));
    assert_eq!(request.limit, Some(123));
    assert_eq!(request.include_counts, Some(true));
}

#[test]
fn request_builder_preserves_multiline_molfile_whitespace() {
    let criteria = SearchCriteria {
        taxon: "*".into(),
        structure: "\n  Mrv\n\n  0  0  0  0  0  0            999 V3000\nM  END\n".into(),
        ..SearchCriteria::up_to_year(YEAR)
    };

    let request = SearchRequest::from_criteria(&criteria, 10, false, YEAR);
    let smiles = request.structure.expect("smiles payload");
    assert!(smiles.starts_with('\n'));
    assert!(smiles.contains("V3000"));
}

#[test]
fn request_builder_omits_the_year_bounds_when_no_year_filter_is_set() {
    let criteria = SearchCriteria {
        taxon: "*".into(),
        ..SearchCriteria::up_to_year(YEAR)
    };

    let request = SearchRequest::from_criteria(&criteria, 10, false, YEAR);
    assert_eq!(request.year_min, None);
    assert_eq!(request.year_max, None);
}

#[test]
fn normalize_statement_strips_wikidata_prefix() {
    assert_eq!(
        normalize_statement(Some(
            "http://www.wikidata.org/entity/statement/S123".to_string()
        )),
        Some("S123".to_string())
    );
    assert_eq!(
        normalize_statement(Some("S124".to_string())),
        Some("S124".to_string())
    );
    assert_eq!(normalize_statement(Some("   ".to_string())), None);
}

#[test]
fn request_builder_sends_trimmed_text_fields_and_omits_blank_ones() {
    let filled = SearchCriteria {
        taxon: "  Gentiana lutea ".into(),
        reference: " 10.1000/xyz ".into(),
        formula_exact: " C15H10O5 ".into(),
        ..SearchCriteria::up_to_year(YEAR)
    };
    let request = SearchRequest::from_criteria(&filled, 10, false, YEAR);
    assert_eq!(request.taxon.as_deref(), Some("Gentiana lutea"));
    assert_eq!(request.reference.as_deref(), Some("10.1000/xyz"));
    assert_eq!(request.formula_exact.as_deref(), Some("C15H10O5"));

    let blank = SearchCriteria {
        taxon: "   ".into(),
        reference: " ".into(),
        formula_exact: "  ".into(),
        ..SearchCriteria::up_to_year(YEAR)
    };
    let request = SearchRequest::from_criteria(&blank, 10, false, YEAR);
    assert_eq!(request.taxon, None);
    assert_eq!(request.reference, None);
    assert_eq!(request.formula_exact, None);
}

#[test]
fn request_builder_sends_a_threshold_only_for_a_similarity_search_with_a_structure() {
    let request_for = |structure: &str, structure_search| {
        let criteria = SearchCriteria {
            taxon: "*".into(),
            structure: structure.into(),
            structure_search,
            structure_threshold: 0.85,
            ..SearchCriteria::up_to_year(YEAR)
        };
        SearchRequest::from_criteria(&criteria, 10, false, YEAR).structure_threshold
    };

    assert_eq!(request_for("CCO", SmilesSearchType::Similarity), Some(0.85));
    assert_eq!(request_for("CCO", SmilesSearchType::Exact), None);
    assert_eq!(request_for("CCO", SmilesSearchType::Substructure), None);
    assert_eq!(request_for("  ", SmilesSearchType::Similarity), None);
}

#[test]
fn server_stats_become_dataset_stats_counter_for_counter() {
    let counted = SearchStats {
        n_compounds: 3,
        n_taxa: 4,
        n_references: 5,
        n_entries: Some(12),
        n_entries_unique: Some(9),
    };
    assert_eq!(
        DatasetStats::from(counted),
        DatasetStats {
            n_compounds: 3,
            n_taxa: 4,
            n_references: 5,
            n_entries: 12,
            n_entries_unique: 9,
        }
    );

    let uncounted = SearchStats {
        n_compounds: 1,
        n_taxa: 1,
        n_references: 1,
        n_entries: Some(7),
        n_entries_unique: None,
    };
    assert_eq!(DatasetStats::from(uncounted).n_entries_unique, 7);
}

#[test]
fn request_builder_serializes_disabled_taxon_nomenclature_switches_only() {
    let default_req =
        SearchRequest::from_criteria(&SearchCriteria::up_to_year(YEAR), 10, false, YEAR);
    assert_eq!(default_req.taxon_accepted_synonyms, None);
    assert_eq!(default_req.taxon_basionyms, None);
    assert_eq!(default_req.taxon_protonyms, None);
    assert_eq!(default_req.taxon_replacements, None);

    let mut narrowed = SearchCriteria::up_to_year(YEAR);
    narrowed.taxon_names.accepted_synonyms = false;
    narrowed.taxon_names.basionyms = false;
    narrowed.taxon_names.protonyms = false;
    narrowed.taxon_names.replacements = false;
    let narrowed_req = SearchRequest::from_criteria(&narrowed, 10, false, YEAR);
    assert_eq!(narrowed_req.taxon_accepted_synonyms, Some(false));
    assert_eq!(narrowed_req.taxon_basionyms, Some(false));
    assert_eq!(narrowed_req.taxon_protonyms, Some(false));
    assert_eq!(narrowed_req.taxon_replacements, Some(false));
}
