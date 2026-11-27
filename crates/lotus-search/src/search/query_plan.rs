// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Structure planning and SPARQL execution-query assembly.

use lotus_model::{SearchCriteria, normalize_structure};
use lotus_query::{Nomenclature, base_query_for_criteria, exact_compounds_query, with_filters};

use super::execute_search::SearchError;
use super::resolve_structure::resolve_inputs;
use super::resolve_taxon::resolve_taxon;
use crate::client::Http;
use crate::result::{SearchRequest, TaxonResolution};

/// The base query for a criteria and an already-resolved taxon.
///
/// `qid` is [`TaxonResolution::qid`](crate::TaxonResolution::qid), and the wildcard `*` means every taxon.
#[must_use]
pub fn build_base_query(criteria: &SearchCriteria, qid: Option<&str>) -> String {
    let structure = normalize_structure(&criteria.structure);
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
        let taxon = qid.or_else(|| {
            (criteria.taxon.trim() == "*" || !criteria.reference.trim().is_empty()).then_some("*")
        });
        let nomenclature = Nomenclature::from(&criteria);
        exact_compounds_query(&resolved.compounds, taxon.map(|t| (t, &nomenclature)))
    } else {
        build_base_query(&criteria, qid)
    };

    with_filters(&base, &criteria, request.year_max)
}

/// Validate criteria, resolve taxon and structure/reference inputs, and assemble
/// the filtered execution query.
pub(super) async fn prepare_query<H: Http>(
    http: &H,
    request: &SearchRequest,
) -> Result<(String, TaxonResolution), SearchError> {
    lotus_model::validate_criteria(&request.criteria, request.year_max)
        .map_err(SearchError::Invalid)?;

    let taxon = resolve_taxon(http, &request.criteria.taxon)
        .await
        .map_err(|e| SearchError::Transport {
            stage: "taxon",
            source: e,
        })?;

    let resolved =
        resolve_inputs(http, &request.criteria)
            .await
            .map_err(|e| SearchError::Transport {
                stage: "structure or reference",
                source: e,
            })?;

    let query = build_execution_query_with(request, taxon.qid.as_deref(), &resolved);
    Ok((query, taxon))
}
