// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! SPARQL query construction. Pure, synchronous, zero-I/O.

// telemetry not available in core
use lotus_model::{SearchCriteria, SmilesSearchType};

/// Normalize a raw SMILES/Molfile string from the criteria.
/// * Line endings are unified to `\n`.
pub fn normalize_smiles(raw: &str) -> String {
    lotus_search::normalize_structure(raw)
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
pub fn build_sparql_query(
    resolved: &ResolvedStructure,
    crit: &SearchCriteria,
    taxon_qid: Option<&str>,
) -> String {
    let nomenclature = lotus_query::Nomenclature::from(crit);

    // Exact means this one compound, answered without the structure service: an
    // index scan on the QID rather than a load and a scan of every candidate. The
    // taxon filter, when there is one, still expands the nomenclature.
    //
    // It needs a compound, which is why resolution runs first for every input
    // kind. Input that named nothing falls through to the service below, where
    // `Exact` becomes the same-molecule search.
    // All the compounds, not just the one the notice names. See `ResolvedStructure`.
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
    let _ = q.contains("SERVICE");
    q
}

/// Apply server-side filters.
pub fn apply_server_filters(base_query: &str, crit: &SearchCriteria, year: u16) -> String {
    lotus_query::with_filters(base_query, crit, year)
}

#[cfg(test)]
#[path = "build_query/tests.rs"]
mod tests;
