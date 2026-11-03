// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `wikidata`, in their own file.

#![allow(clippy::expect_used)]

use super::*;

#[test]
fn normalize_taxon_lookup_trims_and_lowercases() {
    assert_eq!(
        normalize_taxon_lookup("  Gentiana lutea  "),
        Some("gentiana lutea".to_string())
    );
    assert_eq!(normalize_taxon_lookup("   \n"), None);
}

#[test]
fn build_single_taxon_lookup_query_uses_values_without_lcase_filter() {
    let query = build_single_taxon_lookup_query("ficticia imaginaria").expect("query");

    assert!(query.contains("VALUES ?taxonName"));
    assert!(query.contains("\"ficticia imaginaria\" \"Ficticia imaginaria\""));
    assert!(query.contains("wdt:P31 wd:Q16521"));
    assert!(!query.contains("LCASE"));
    assert!(!query.contains("FILTER"));
}
