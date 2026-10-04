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

/// What the structure field resolved to.
///
/// Both parts matter and neither is optional, because the search mode needs both
/// to answer: the compound for an identity search, and the structure for the two
/// modes that call the structure service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStructure {
    /// The Wikidata compound the input resolved to, when it resolved to one.
    ///
    /// `None` means the input named nothing — a SMILES Wikidata does not have,
    /// which is a perfectly good structure. That is the only way to get `None`; a
    /// name, an `InChIKey` or a QID that matched nothing is an error instead.
    pub compound: Option<String>,
    /// Every compound the input resolved to, searched together.
    ///
    /// Empty exactly when [`Self::compound`] is `None`. A structure names a
    /// molecule and the structure service answers with everything inside the
    /// cutoff, so this is normally several QIDs -- the stereoisomers of what was
    /// typed. `compound` is the one named in the notice; this is the set the
    /// query asks about.
    pub compounds: Vec<String>,
    /// The literal the structure service is handed.
    ///
    /// The resolved compound's canonical SMILES (`P233`) when there is one, and
    /// the reader's own input otherwise. A substructure or similarity search on a
    /// name therefore searches the compound's own structure rather than the text
    /// that named it, which is the only thing those two modes could mean.
    pub structure: String,
}

impl ResolvedStructure {
    /// Input that resolved to nothing: the structure, exactly as typed.
    #[must_use]
    pub fn unresolved(structure: &str) -> Self {
        Self {
            compound: None,
            compounds: Vec::new(),
            structure: structure.to_owned(),
        }
    }
}

/// Build the base SPARQL query for the given criteria, resolved taxon QID, and
/// what the structure field resolved to.
pub fn build_sparql_query(
    resolved: &ResolvedStructure,
    crit: &SearchCriteria,
    taxon_qid: Option<&str>,
) -> String {
    let nomenclature = lotus_query::Nomenclature::from(crit);

    // Exact means this one compound, and it is answered without the structure
    // service: an index scan on the QID rather than a load and a scan of every
    // candidate compound. The taxon filter, when there is one, still expands the
    // nomenclature.
    //
    // It needs a compound, which is why the resolution runs first for every input
    // kind. Input that named nothing falls through to the service below, where
    // `Exact` becomes the same-molecule search.
    // All the compounds, not the one the notice names. See `ResolvedStructure`.
    if crit.structure_search == SmilesSearchType::Exact && !resolved.compounds.is_empty() {
        let taxon = taxon_qid.and_then(|t| (t != "*").then_some((t, &nomenclature)));
        return lotus_query::exact_compounds_query(&resolved.compounds, taxon);
    }

    let structure = resolved.structure.as_str();
    if structure.is_empty() {
        return match taxon_qid {
            Some(qid) if qid != "*" => {
                lotus_query::compounds_by_taxon_query_with(qid, &nomenclature)
            }
            _ => lotus_query::all_compounds_query(),
        };
    }

    // The mode is used as asked for, molfiles included: the similarity service
    // was measured accepting a multi-line CTAB, so the format does not decide it.
    let taxon_for_sachem = match taxon_qid {
        Some("*") => Some("Q2382443"),
        Some(qid) => Some(qid),
        None => None,
    };
    let q = lotus_query::structure_search_query_with(
        structure,
        crit.structure_search,
        crit.structure_threshold,
        taxon_for_sachem,
        &nomenclature,
    );
    telemetry::query_build_sachem_query_created(q.contains("SERVICE"));
    q
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
        // The common case for a SMILES: not a compound Wikidata has. The exact
        // mode then means the same molecule, which is the only answer the service
        // can give about a structure.
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
    /// The bug this is about: `pick` took the first match, and for
    /// `C[C@H](O)CO` that is the achiral `Q161495`, which has no occurrence in
    /// Wikidata, while `Q27093218` -- the (R) form that was actually typed --
    /// has six. Asking only the first asked an arbitrary question.
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
        // The mode is a question about *other* compounds, so it overrides the
        // resolution: it needs the service, and it gets the compound's own
        // structure rather than the text that named it.
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
}
