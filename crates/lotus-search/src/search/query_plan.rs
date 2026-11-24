// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Structure planning and SPARQL execution-query assembly.

use lotus_model::{SearchCriteria, SmilesSearchType, normalize_structure};
use lotus_query::{Nomenclature, base_query_for_criteria, exact_compounds_query, with_filters};

use crate::result::SearchRequest;

/// How a structure search should be run, once the structure is known.
#[derive(Debug, Clone, PartialEq)]
pub struct StructurePlan {
    /// The normalised structure text.
    pub structure: String,
    /// The search to run, as asked for. A molfile takes either mode: the
    /// similarity service was measured accepting a multi-line CTAB literal and
    /// answering a cutoff search, so the format does not decide the mode.
    pub search: SmilesSearchType,
    /// Tanimoto cutoff, when `search` is a similarity search.
    pub threshold: f64,
    /// Whether the structure is a molfile rather than a SMILES string.
    pub is_molfile: bool,
}

impl StructurePlan {
    /// Whether the request is a structure search at all.
    #[must_use]
    pub fn for_request(criteria: &SearchCriteria) -> Option<Self> {
        let structure = normalize_structure(&criteria.structure);
        if structure.is_empty() {
            return None;
        }
        let is_molfile = lotus_model::classify_structure(&structure).is_molfile();
        Some(Self {
            structure,
            search: criteria.structure_search,
            threshold: criteria.structure_threshold,
            is_molfile,
        })
    }
}

/// The base query for a criteria and an already-resolved taxon.
///
/// `qid` is [`TaxonResolution::qid`](crate::TaxonResolution::qid), and the wildcard `*` means every taxon.
#[must_use]
pub fn build_base_query(criteria: &SearchCriteria, qid: Option<&str>) -> String {
    let structure = StructurePlan::for_request(criteria).map_or_default(|plan| plan.structure);
    base_query_for_criteria(criteria, &structure, qid)
}

/// The query that will actually be sent: the base query plus the active filters.
#[must_use]
pub fn build_execution_query(request: &SearchRequest, qid: Option<&str>) -> String {
    build_execution_query_with(request, qid, &ResolvedInputs::default())
}

/// What resolution found, for the query builder to use instead of the raw input.
///
/// A separate parameter rather than a rewritten `SearchCriteria`, because the two
/// say different things: `criteria.structure` is what the reader typed, and a name
/// that identified a compound must still be searched *by that compound* rather
/// than by the text.
#[derive(Debug, Clone, Default)]
pub struct ResolvedInputs {
    /// Compound QIDs a structure field resolved to. Non-empty only when the
    /// structure was something other than a SMILES the service could take.
    pub compounds: Vec<String>,
    /// The canonical SMILES a compound name or `InChIKey` resolved to, used when
    /// a substructure or similarity search needs a structure literal for the
    /// service rather than the identifier or name the reader typed.
    pub smiles: Option<String>,
    /// The reference QID a DOI resolved to.
    pub reference_qid: Option<String>,
}

/// Build the query with resolution applied.
///
/// An exact structure search that resolved to compounds goes by QID: the client's
/// rule, and the reason `--structure ethanol --structure-search exact` used to
/// fail. Handing the *name* to the structure service is a 500, because a name is
/// not a structure. Everything else takes the existing route with the canonical
/// SMILES substituted for the input.
#[must_use]
pub fn build_execution_query_with(
    request: &SearchRequest,
    qid: Option<&str>,
    resolved: &ResolvedInputs,
) -> String {
    let mut criteria = request.criteria.clone();
    if let Some(reference) = &resolved.reference_qid {
        criteria.reference.clone_from(reference);
    }
    if let Some(smiles) = &resolved.smiles
        && !smiles.is_empty()
    {
        criteria.structure.clone_from(smiles);
    }

    let base = if !resolved.compounds.is_empty()
        && criteria.structure_search == lotus_model::SmilesSearchType::Exact
    {
        let taxon = match qid {
            Some("*") => Some("Q2382443"),
            other => other,
        };
        let nomenclature = Nomenclature::from(&criteria);
        exact_compounds_query(&resolved.compounds, taxon.map(|t| (t, &nomenclature)))
    } else {
        build_base_query(&criteria, qid)
    };

    with_filters(&base, &criteria, request.year_max)
}
