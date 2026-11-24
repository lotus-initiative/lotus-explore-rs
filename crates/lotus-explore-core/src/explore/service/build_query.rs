// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! SPARQL query construction. Pure, synchronous, zero-I/O.

// telemetry not available in core
use lotus_model::{SearchCriteria, SmilesSearchType};

/// Normalize a raw SMILES/Molfile string from the criteria.
/// * Line endings are unified to `\n`.
#[must_use]
pub fn normalize_smiles(raw: &str) -> String {
    lotus_model::normalize_structure(raw)
}

/// What the structure field resolved to.
///
/// The search mode needs both parts to answer: the compound for an identity
/// search, the structure for the two modes that call the structure service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStructure {
    /// The Wikidata compound the input resolved to, when it resolved to one.
    ///
    /// `None` means the input named nothing — a SMILES Wikidata does not have,
    /// which is a perfectly good structure. That is the only way to get `None`; a
    /// name, an `InChIKey` or a QID matching nothing is an error instead.
    pub compound: Option<String>,
    /// Every compound the input resolved to, searched together.
    ///
    /// Empty exactly when [`Self::compound`] is `None`. A structure names a molecule
    /// and the service answers with everything inside the cutoff, so this is
    /// normally several QIDs — the stereoisomers of what was typed. `compound` is
    /// the one named in the notice; this is the set the query asks about.
    pub compounds: Vec<String>,
    /// The literal the structure service is handed.
    ///
    /// The resolved compound's canonical SMILES (`P233`) when there is one, else the
    /// reader's own input. A substructure or similarity search on a name therefore
    /// searches the compound's own structure, the only thing those modes could
    /// mean.
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
#[must_use]
pub fn build_sparql_query(
    resolved: &ResolvedStructure,
    crit: &SearchCriteria,
    taxon_qid: Option<&str>,
) -> String {
    if crit.structure_search == SmilesSearchType::Exact && !resolved.compounds.is_empty() {
        let nomenclature = lotus_query::Nomenclature::from(crit);
        let taxon = taxon_qid.and_then(|t| (t != "*").then_some((t, &nomenclature)));
        return lotus_query::exact_compounds_query(&resolved.compounds, taxon);
    }

    // The mode is used as asked for, molfiles included: the similarity service
    // was measured accepting a multi-line CTAB, so the format does not decide it.
    // `Some("*")` passes straight through so `structure_search_query_with`
    // requires an occurrence without expanding a `P171*` closure over Biota.
    lotus_query::base_query_for_criteria(crit, resolved.structure.as_str(), taxon_qid)
}

/// Apply server-side filters.
#[must_use]
pub fn apply_server_filters(base_query: &str, crit: &SearchCriteria, year: u16) -> String {
    lotus_query::with_filters(base_query, crit, year)
}

#[cfg(test)]
#[path = "build_query/tests.rs"]
mod tests;
