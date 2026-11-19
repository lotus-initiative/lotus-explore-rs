// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Unit tests for `query`.

use super::*;

#[test]
fn taxon_has_compounds_query_limits_occurrence_probe_to_one_row() {
    let probe = taxon_has_compounds_query("Q158572");
    assert_eq!(
        probe,
        limit_query(&occurrences_query(&compounds_by_taxon_query("Q158572")), 1)
    );
    assert!(probe.contains("wd:Q158572"));
    assert!(probe.contains("LIMIT 1"));
}

#[test]
fn has_data_row_detects_non_empty_row_after_header() {
    assert!(!has_data_row(b""));
    assert!(!has_data_row(b"a,b"));
    assert!(!has_data_row(b"a,b\n"));
    assert!(!has_data_row(b"a,b\n\n"));
    assert!(has_data_row(b"a,b\n1,2\n"));
    assert!(has_data_row(b"a,b\n\n1,2"));
}

#[test]
fn occurrence_queries_exclude_deprecated_p703_statements() {
    let taxon_query = compounds_by_taxon_query("Q158572");
    assert!(taxon_query.contains("wikibase:DeprecatedRank"));
    assert!(taxon_query.contains("FILTER(?_stmt_rank != wikibase:DeprecatedRank)"));

    let optional_query = optional_compounds_query_body("?c wdt:P235 ?compound_inchikey .", "");
    assert!(optional_query.contains("FILTER(?_stmt_rank != wikibase:DeprecatedRank)"));

    let seeded = exact_compound_query("Q16634054", None);
    assert!(seeded.contains("FILTER(?_stmt_rank != wikibase:DeprecatedRank)"));

    let seeded_with_taxon =
        exact_compound_query("Q16634054", Some(("Q158572", &Nomenclature::ALL_ON)));
    assert!(seeded_with_taxon.contains("FILTER(?_stmt_rank != wikibase:DeprecatedRank)"));

    let structure = structure_search_query_with(
        "CC(=O)O",
        lotus_model::SmilesSearchType::Substructure,
        0.8,
        None,
        &Nomenclature::ALL_ON,
    );
    assert!(structure.contains("FILTER(?_stmt_rank != wikibase:DeprecatedRank)"));

    let structure_with_taxon = structure_search_query_with(
        "CC(=O)O",
        lotus_model::SmilesSearchType::Substructure,
        0.8,
        Some("Q158572"),
        &Nomenclature::ALL_ON,
    );
    assert!(structure_with_taxon.contains("FILTER(?_stmt_rank != wikibase:DeprecatedRank)"));

    let all_q = all_required_compounds_query_body();
    assert!(all_q.contains("FILTER(?_stmt_rank != wikibase:DeprecatedRank)"));

    let large_qids = vec![1; DICTIONARY_VALUES_LIMIT + 1];
    assert!(
        compound_dictionary_query(&large_qids).contains("FILTER(?_sr != wikibase:DeprecatedRank)")
    );
    assert!(
        reference_dictionary_query(&large_qids).contains("FILTER(?_sr != wikibase:DeprecatedRank)")
    );

    assert!(crate::bulk_export::COMPOUND_TAXON_REFERENCE_QUERY.contains("wikibase:DeprecatedRank"));
}

#[test]
fn tagged_name_values_emits_deduplicated_case_variants() {
    let mixed = tagged_name_values("Carnosol");
    assert!(mixed.contains(r#""Carnosol"@en"#), "{mixed}");
    assert!(mixed.contains(r#""carnosol"@en"#), "{mixed}");
    assert_eq!(mixed.matches(r#""Carnosol"@en"#).count(), 1, "{mixed}");

    let lower = tagged_name_values("carnosol");
    assert!(lower.contains(r#""carnosol"@en"#), "{lower}");
    assert!(lower.contains(r#""Carnosol"@en"#), "{lower}");
    assert_eq!(lower.matches(r#""carnosol"@en"#).count(), 1, "{lower}");

    let locant = tagged_name_values("12-O-Methylcarnosic acid");
    assert!(
        locant.contains(r#""12-O-Methylcarnosic acid"@en"#),
        "{locant}"
    );
    assert!(
        locant.contains(r#""12-o-methylcarnosic acid"@en"#),
        "{locant}"
    );
    assert_eq!(
        locant.matches(r#""12-o-methylcarnosic acid"@en"#).count(),
        1,
        "{locant}"
    );
}
