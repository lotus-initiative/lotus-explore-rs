// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `build_query`, in their own file.

use super::*;
use lotus_model::SmilesSearchType;
use lotus_search::SearchCriteria;

#[test]
fn empty_smiles_and_no_taxon_returns_all_compounds_query() {
    let crit = SearchCriteria::up_to_year(crate::clock::current_year());
    let q = build_sparql_query(&ResolvedStructure::unresolved(""), &crit, None);
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
    let q = build_sparql_query(&ResolvedStructure::unresolved(""), &crit, Some("Q156598"));
    assert!(q.contains("Q156598"), "query must reference the taxon QID");
}

#[test]
fn wild_star_taxon_qid_generates_all_compounds_query() {
    let crit = SearchCriteria::up_to_year(crate::clock::current_year());
    let q = build_sparql_query(&ResolvedStructure::unresolved(""), &crit, Some("*"));
    assert!(!q.contains("Q156598"), "wildcard must not filter by QID");
}

#[test]
fn a_structure_that_resolved_to_nothing_still_calls_the_service() {
    // The common case for a SMILES: not a compound Wikidata has. `Exact` then
    // means the same molecule, the only answer the service can give about a
    // structure.
    let crit = SearchCriteria {
        structure: "c1ccccc1".into(),
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };
    let q = build_sparql_query(&ResolvedStructure::unresolved("c1ccccc1"), &crit, None);
    assert!(q.contains("SERVICE"), "expected the structure service: {q}");
    assert!(
        q.contains(r#"sachem:cutoff "1"^^xsd:double"#),
        "exact means an identical fingerprint: {q}"
    );
}

#[test]
fn an_exact_search_of_a_resolved_compound_never_calls_the_service() {
    let crit = SearchCriteria {
        structure: "amarogentina".into(),
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };
    let resolved = ResolvedStructure {
        compounds: vec!["Q3613679".into()],
        compound: Some("Q3613679".into()),
        structure: "CC1=C2C(=CC=C1)C(=COC2=O)O".into(),
    };
    let q = build_sparql_query(&resolved, &crit, None);
    assert!(q.contains("VALUES ?c { wd:Q3613679 }"), "{q}");
    assert!(!q.contains("SERVICE"), "exact must not call it: {q}");
}

/// Every match the structure service returned is searched, not just the one
/// the notice names.
///
/// The bug this is about: `pick` took the first match, and for `C[C@H](O)CO` that
/// is the achiral `Q161495`, which has no occurrence in Wikidata, while
/// `Q27093218` -- the (R) form actually typed -- has six. Asking only the first
/// asked an arbitrary question.
#[test]
fn an_exact_search_asks_about_every_compound_the_structure_resolved_to() {
    let crit = SearchCriteria {
        structure: "C[C@H](O)CO".into(),
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };
    let resolved = ResolvedStructure {
        compounds: vec!["Q161495".into(), "Q27093218".into(), "Q27095160".into()],
        compound: Some("Q161495".into()),
        structure: "C[C@H](O)CO".into(),
    };
    let q = build_sparql_query(&resolved, &crit, None);
    for qid in ["Q161495", "Q27093218", "Q27095160"] {
        assert!(
            q.contains(&format!("wd:{qid}")),
            "{qid} is missing from: {q}"
        );
    }
    // One request, not one per candidate.
    assert_eq!(
        q.matches("VALUES ?c {").count(),
        2,
        "the seed list is emitted once per position in the query, not once \
         per compound"
    );
}

#[test]
fn a_substructure_search_of_a_resolved_compound_does_call_the_service() {
    // A question about *other* compounds, so it overrides the resolution: it needs
    // the service, and gets the compound's own structure, not the text that
    // named it.
    let crit = SearchCriteria {
        structure: "amarogentina".into(),
        structure_search: SmilesSearchType::Substructure,
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };
    let resolved = ResolvedStructure {
        compounds: Vec::new(),
        compound: Some("Q3613679".into()),
        structure: "CC1=C2C(=CC=C1)C(=COC2=O)O".into(),
    };
    let q = build_sparql_query(&resolved, &crit, None);
    assert!(q.contains("sachem:substructureSearch"), "{q}");
    assert!(
        q.contains("CC1=C2C(=CC=C1)C(=COC2=O)O"),
        "the compound's canonical SMILES is what is searched: {q}"
    );
}

#[test]
fn a_similarity_search_of_a_resolved_compound_carries_its_cutoff() {
    let crit = SearchCriteria {
        structure: "Q23118".into(),
        structure_search: SmilesSearchType::Similarity,
        structure_threshold: 0.9,
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };
    let resolved = ResolvedStructure {
        compounds: Vec::new(),
        compound: Some("Q23118".into()),
        structure: "CCN(CC)C(=O)C1=CN(C2=CC=CC=C12)C".into(),
    };
    let q = build_sparql_query(&resolved, &crit, None);
    assert!(q.contains("sachem:similarCompoundSearch"), "{q}");
    assert!(q.contains(r#"sachem:cutoff "0.9"^^xsd:double"#), "{q}");
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
