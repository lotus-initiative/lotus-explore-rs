// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! SPARQL query construction, taxon resolution, and request transformation logic.
#![expect(
    clippy::redundant_pub_crate,
    reason = "private crate-root module uses pub(crate) because workspace denies unreachable_pub"
)]

use crate::{
    errors::ApiError,
    state::{AppState, taxon_cache_get, taxon_cache_put},
    types::SearchRequest,
};
use lotus_model::SearchCriteria;

pub(crate) fn apply_request(req: &SearchRequest) -> Result<SearchCriteria, ApiError> {
    let mut c = SearchCriteria {
        taxon: req.taxon.clone().unwrap_or_default(),
        reference: req.reference.clone().unwrap_or_default(),
        structure: req.smiles.clone().unwrap_or_default(),
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };

    check_reference(&c.reference)?;

    if let Some(v) = req.taxon_accepted_synonyms {
        c.taxon_names.accepted_synonyms = v;
    }
    if let Some(v) = req.taxon_basionyms {
        c.taxon_names.basionyms = v;
    }
    if let Some(v) = req.taxon_protonyms {
        c.taxon_names.protonyms = v;
    }
    if let Some(v) = req.taxon_replacements {
        c.taxon_names.replacements = v;
    }
    if let Some(v) = req.smiles_search_type {
        c.structure_search = v.into();
    }
    if let Some(v) = req.similarity_threshold {
        if v <= 0.0 {
            return Err(ApiError::bad_request(
                "similarity_threshold must be greater than 0",
            ));
        }
        c.structure_threshold = v.clamp(0.05, 1.0);
    }
    if let Some(v) = req.mass_min {
        c.mass_min = v;
    }
    if let Some(v) = req.mass_max {
        c.mass_max = v;
    }
    if let Some(v) = req.year_min {
        c.year_min = v;
    }
    if let Some(v) = req.year_max {
        c.year_max = v;
    }

    let has_formula_input = req
        .formula_exact
        .as_deref()
        .is_some_and(|v| !v.trim().is_empty())
        || req.c_min.is_some()
        || req.c_max.is_some()
        || req.h_min.is_some()
        || req.h_max.is_some()
        || req.n_min.is_some()
        || req.n_max.is_some()
        || req.o_min.is_some()
        || req.o_max.is_some()
        || req.p_min.is_some()
        || req.p_max.is_some()
        || req.s_min.is_some()
        || req.s_max.is_some()
        || req.f_state.is_some()
        || req.cl_state.is_some()
        || req.br_state.is_some()
        || req.i_state.is_some();

    c.formula_enabled = has_formula_input;
    if let Some(v) = req.formula_exact.as_deref() {
        c.formula_exact = v.trim().to_string();
    }

    macro_rules! apply_opt {
        ($src:expr => $dst:expr) => {
            if let Some(v) = $src {
                $dst = v;
            }
        };
    }
    apply_opt!(req.c_min => c.c_min);
    apply_opt!(req.c_max => c.c_max);
    apply_opt!(req.h_min => c.h_min);
    apply_opt!(req.h_max => c.h_max);
    apply_opt!(req.n_min => c.n_min);
    apply_opt!(req.n_max => c.n_max);
    apply_opt!(req.o_min => c.o_min);
    apply_opt!(req.o_max => c.o_max);
    apply_opt!(req.p_min => c.p_min);
    apply_opt!(req.p_max => c.p_max);
    apply_opt!(req.s_min => c.s_min);
    apply_opt!(req.s_max => c.s_max);

    if c.c_min > c.c_max
        || c.h_min > c.h_max
        || c.n_min > c.n_max
        || c.o_min > c.o_max
        || c.p_min > c.p_max
        || c.s_min > c.s_max
    {
        return Err(ApiError::bad_request("Element min must be <= max"));
    }

    apply_opt!(req.f_state.map(Into::into)  => c.f_state);
    apply_opt!(req.cl_state.map(Into::into) => c.cl_state);
    apply_opt!(req.br_state.map(Into::into) => c.br_state);
    apply_opt!(req.i_state.map(Into::into)  => c.i_state);

    validated(c)
}

fn validated(criteria: SearchCriteria) -> Result<SearchCriteria, ApiError> {
    lotus_model::validate_criteria(&criteria, crate::clock::current_year())
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(criteria)
}

fn check_reference(reference: &str) -> Result<(), ApiError> {
    let trimmed = reference.trim();
    if trimmed.is_empty()
        || lotus_model::looks_like_reference_qid(trimmed)
        || lotus_model::looks_like_doi(trimmed)
    {
        return Ok(());
    }
    Err(ApiError::bad_request(
        "reference must be a Wikidata QID (such as Q23118) or a DOI (such as 10.7554/eLife.70780)",
    ))
}

#[cfg(test)]
pub(crate) fn build_execution_query(
    criteria: &SearchCriteria,
    resolved_taxon_qid: Option<&str>,
) -> String {
    build_execution_query_with(
        criteria,
        resolved_taxon_qid,
        &lotus_search::ResolvedInputs::default(),
    )
}

/// Delegates directly to [`lotus_search::build_execution_query_with`] so the
/// server, CLI, and browser answer the same request with the same SPARQL.
pub(crate) fn build_execution_query_with(
    criteria: &SearchCriteria,
    resolved_taxon_qid: Option<&str>,
    resolved_inputs: &lotus_search::ResolvedInputs,
) -> String {
    let request = lotus_search::SearchRequest::new(criteria.clone(), crate::clock::current_year());
    lotus_search::build_execution_query_with(&request, resolved_taxon_qid, resolved_inputs)
}

pub(crate) async fn resolve_taxon_qid_cached(
    state: &AppState,
    taxon_input: String,
) -> Result<(Option<String>, Option<String>), ApiError> {
    let key = taxon_input.trim().to_lowercase();
    if !key.is_empty()
        && let Some(cached) = taxon_cache_get(state, &key)
    {
        return Ok(cached);
    }

    let http = lotus_search::reqwest_client::ReqwestClient::new()
        .map_err(|e| ApiError::upstream(format!("HTTP client init failed: {e}")))?;
    let resolved = resolve_taxon_with(&http, &taxon_input).await?;
    if !key.is_empty() {
        taxon_cache_put(state, key, resolved.clone());
    }
    Ok(resolved)
}

pub(crate) async fn resolve_taxon_with<H: lotus_search::Http>(
    http: &H,
    taxon_input: &str,
) -> Result<(Option<String>, Option<String>), ApiError> {
    let trimmed = taxon_input.trim();
    if trimmed == "*" {
        return Ok((Some("*".into()), None));
    }
    let resolution = lotus_search::resolve_taxon(http, trimmed)
        .await
        .map_err(|err| match err {
            lotus_search::FetchError::Parse(msg) if msg.starts_with("no taxon matched ") => {
                ApiError::bad_request(format!("Taxon not found: {trimmed}"))
            }
            other => ApiError::from_fetch("taxon lookup failed", other),
        })?;

    let warning = (!resolution.notes.is_empty()).then(|| {
        resolution
            .notes
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    });
    Ok((resolution.qid, warning))
}

pub(crate) async fn resolve_search_inputs(
    criteria: &SearchCriteria,
) -> Result<lotus_search::ResolvedInputs, ApiError> {
    let http = lotus_search::reqwest_client::ReqwestClient::new()
        .map_err(|e| ApiError::upstream(format!("HTTP client init failed: {e}")))?;
    resolve_search_inputs_with(&http, criteria).await
}

pub(crate) async fn resolve_search_inputs_with<H: lotus_search::Http>(
    http: &H,
    criteria: &SearchCriteria,
) -> Result<lotus_search::ResolvedInputs, ApiError> {
    lotus_search::resolve_inputs(http, criteria)
        .await
        .map_err(|err| match err {
            lotus_search::FetchError::Parse(msg) if msg.starts_with("no ") => {
                ApiError::bad_request(msg)
            }
            other => ApiError::from_fetch("input resolution failed", other),
        })
}
