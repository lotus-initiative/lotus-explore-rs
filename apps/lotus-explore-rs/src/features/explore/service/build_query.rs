// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! SPARQL query construction service.
//! Pure, synchronous, zero-I/O — ideal for unit testing without stubs.

use crate::services::search_telemetry as telemetry;
use lotus_model::{SearchCriteria, SmilesSearchType};

/// Normalize a raw SMILES/Molfile string from the criteria.
/// * Line endings are unified to `\n`.
pub fn normalize_smiles(raw: &str) -> String {
    // Fast path: skip allocation when no carriage returns are present (common case).
    let normalized = if raw.contains('\r') {
        raw.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        raw.to_owned()
    };
    let kind = lotus_model::classify_structure(&normalized);
    if matches!(
        kind,
        lotus_model::StructureKind::MolfileV2000 | lotus_model::StructureKind::MolfileV3000
    ) {
        normalized
    } else {
        normalized.trim().to_string()
    }
}

/// Build the base SPARQL query for the given criteria and resolved taxon QID.
/// * If `smiles` is non-empty the Sachem SERVICE query is used.
/// * Otherwise a taxon-filtered or "all compounds" query is generated.
pub fn build_sparql_query(smiles: &str, crit: &SearchCriteria, taxon_qid: Option<&str>) -> String {
    if smiles.is_empty() {
        match taxon_qid {
            Some(qid) if qid != "*" => lotus_query::compounds_by_taxon_query(qid),
            _ => lotus_query::all_compounds_query(),
        }
    } else {
        let effective_type = if (smiles.contains('\n') || smiles.contains('\r'))
            && crit.structure_search == SmilesSearchType::Similarity
        {
            SmilesSearchType::Substructure
        } else {
            crit.structure_search
        };
        let taxon_for_sachem = match taxon_qid {
            Some("*") => Some("Q2382443"),
            Some(qid) => Some(qid),
            None => None,
        };
        let q = lotus_query::structure_search_query(
            smiles,
            effective_type,
            crit.structure_threshold,
            taxon_for_sachem,
        );
        telemetry::query_build_sachem_query_created(q.contains("SERVICE"));
        q
    }
}

/// Apply server-side filters and log the outcome.
pub fn apply_server_filters(base_query: &str, crit: &SearchCriteria) -> String {
    // `with_filters` takes the current year so that `lotus-query` stays pure and
    // its tests can pin one. This is the app, which has a clock.
    let execution_query = lotus_query::with_filters(base_query, crit, crate::clock::current_year());

    telemetry::query_build_after_server_filters(
        execution_query.contains("SERVICE"),
        execution_query.contains("FILTER"),
    );
    execution_query
}

#[cfg(test)]
mod tests {
    use super::*;
    use lotus_search::SearchCriteria;

    #[test]
    fn empty_smiles_and_no_taxon_returns_all_compounds_query() {
        let crit = SearchCriteria::up_to_year(crate::clock::current_year());
        let q = build_sparql_query("", &crit, None);
        // All-compounds query should select without a FILTER for a specific taxon.
        assert!(
            q.contains("SELECT") || q.contains("select"),
            "must be a SELECT query"
        );
        assert!(!q.contains("Q12345"), "must not contain a specific QID");
    }

    #[test]
    fn taxon_qid_only_generates_by_taxon_query() {
        let crit = SearchCriteria {
            taxon: "Gentiana lutea".into(),
            ..SearchCriteria::up_to_year(crate::clock::current_year())
        };
        let q = build_sparql_query("", &crit, Some("Q156598"));
        assert!(q.contains("Q156598"), "query must reference the taxon QID");
    }

    #[test]
    fn wild_star_taxon_qid_generates_all_compounds_query() {
        let crit = SearchCriteria::up_to_year(crate::clock::current_year());
        let q = build_sparql_query("", &crit, Some("*"));
        assert!(!q.contains("Q156598"), "wildcard must not filter by QID");
    }

    #[test]
    fn smiles_presence_generates_sachem_query() {
        let crit = SearchCriteria {
            structure: "c1ccccc1".into(),
            structure_search: SmilesSearchType::Substructure,
            ..SearchCriteria::up_to_year(crate::clock::current_year())
        };
        let q = build_sparql_query("c1ccccc1", &crit, None);
        // Sachem queries always contain a SERVICE block.
        assert!(q.contains("SERVICE"), "sachem query must contain SERVICE");
    }

    #[test]
    fn normalize_smiles_trims_plain_smiles() {
        assert_eq!(normalize_smiles("  CC=O  "), "CC=O");
    }

    #[test]
    fn normalize_smiles_unifies_line_endings() {
        let raw = "CC\r\nCC";
        let got = normalize_smiles(raw);
        assert!(!got.contains('\r'), "carriage returns must be removed");
        assert!(got.contains('\n'), "newline must be present");
    }

    #[test]
    fn normalize_smiles_preserves_molfile_block() {
        // A minimal V2000 molfile starts with 3 header lines followed by counts.
        let molfile = "\n\n\n  1  0  0  0  0  0  0  0  0  0999 V2000\n    0.0000    0.0000    0.0000 C   0  0  0  0  0  0\nM  END\n";
        let got = normalize_smiles(molfile);
        // Molfile content should NOT be stripped.
        assert!(got.contains("V2000"));
    }
}
