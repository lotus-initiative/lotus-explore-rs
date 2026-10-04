// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! SPARQL query construction, taxon resolution, and request transformation logic.
//! This module bridges the HTTP request layer and the upstream SPARQL endpoint:
//! More detail in the type and function docs below.

use crate::server::errors::ApiError;
use crate::server::state::{AppState, taxon_cache_get, taxon_cache_put};
use crate::server::types::SearchRequest;
use crate::sparql;
use flate2::{Compression, write::GzEncoder};
use lotus_model::{SearchCriteria, TaxonMatch};

pub fn apply_request(req: &SearchRequest) -> Result<SearchCriteria, ApiError> {
    let mut c = SearchCriteria {
        taxon: req.taxon.clone().unwrap_or_default(),
        reference: req.reference.clone().unwrap_or_default(),
        structure: req.smiles.clone().unwrap_or_default(),
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };

    // Absent leaves the model default alone, so a caller that omits the
    // toggles gets the same answer as the UI's default checkboxes.
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
        c.mass_min = v.max(0.0);
    }
    if let Some(v) = req.mass_max {
        c.mass_max = v.max(c.mass_min);
    }
    if let Some(v) = req.year_min {
        c.year_min = v;
    }
    if let Some(v) = req.year_max {
        c.year_max = v.max(c.year_min);
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

    // Apply optional element-count bounds from the request.
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

    // Halogen presence states — each converts via `Into` from the request enum.
    apply_opt!(req.f_state.map(Into::into)  => c.f_state);
    apply_opt!(req.cl_state.map(Into::into) => c.cl_state);
    apply_opt!(req.br_state.map(Into::into) => c.br_state);
    apply_opt!(req.i_state.map(Into::into)  => c.i_state);

    Ok(c)
}

/// The query the API sends, which is the query the browser sends.
///
/// This used to be a second implementation of the same dispatch that
/// `lotus_search::build_base_query` performs, and the two disagreed. The
/// divergence was not hypothetical: the server resolved a wildcard to
/// `Some("*")` where the library resolves it to `None`, so it had to match both
/// together -- and `None` is also what an *empty* taxon box produces. The result
/// was that an empty taxon box ran `all_compounds_query`, which requires `P703`,
/// while the same request in the browser ran
/// `all_compounds_including_untaxonomised_query`, which does not.
///
/// So the API answered the narrower question for a blank box -- the exact bug
/// commit 4ead47a set out to fix, fixed in the library and still live here. Two
/// implementations of one dispatch is what let that happen.
///
/// One implementation now. The wildcard still arrives as `Some("*")`, which the
/// library's own match handles: `Some(_) => all_compounds_query()`, so the API
/// keeps requiring an occurrence for `*` and gains the broader answer for a blank
/// box, agreeing with the browser on both.
pub fn build_execution_query(
    criteria: &SearchCriteria,
    resolved_taxon_qid: Option<&str>,
) -> String {
    let request = lotus_search::SearchRequest::new(criteria.clone(), crate::clock::current_year());
    lotus_search::build_execution_query(&request, resolved_taxon_qid)
}

pub async fn resolve_taxon_qid_cached(
    state: &AppState,
    taxon_input: String,
) -> Result<(Option<String>, Option<String>), ApiError> {
    let key = taxon_input.trim().to_lowercase();
    if !key.is_empty()
        && let Some(cached) = taxon_cache_get(state, &key)
    {
        return Ok(cached);
    }

    let resolved = resolve_taxon_qid(taxon_input).await?;
    if !key.is_empty() {
        taxon_cache_put(state, key, resolved.clone());
    }
    Ok(resolved)
}

async fn resolve_taxon_qid(
    taxon_input: String,
) -> Result<(Option<String>, Option<String>), ApiError> {
    let taxon = taxon_input.trim();
    if taxon.is_empty() {
        return Ok((None, None));
    }
    if taxon == "*" {
        return Ok((Some("*".into()), None));
    }
    if lotus_search::is_qid(taxon) {
        return Ok((Some(taxon.to_ascii_uppercase()), None));
    }

    let sanitized = lotus_search::standardize_taxon_name(taxon);
    let query = lotus_query::taxon_lookup_query(&sanitized);
    let csv = sparql::execute_sparql_bytes(&query)
        .await
        .map_err(|e| ApiError::upstream(format!("taxon lookup failed: {e}")))?;
    let matches = lotus_query::parse_taxon_csv(&csv)
        .map_err(|e| ApiError::upstream(format!("taxon parse failed: {e}")))?;

    if matches.is_empty() {
        return Err(ApiError::bad_request(format!("Taxon not found: {taxon}")));
    }

    let lower = sanitized.to_lowercase();
    let exact: Vec<&TaxonMatch> = matches
        .iter()
        .filter(|m| m.name.to_lowercase() == lower)
        .collect();
    let best = exact
        .first()
        .copied()
        .or_else(|| matches.first())
        .ok_or_else(|| ApiError::bad_request("Could not resolve taxon"))?;

    let warning = if sanitized != taxon {
        Some(format!(
            "Taxon normalized from '{taxon}' to '{}' ({}).",
            best.name, best.qid
        ))
    } else if exact.len() > 1 || (exact.is_empty() && matches.len() > 1) {
        Some(format!(
            "Ambiguous taxon input. Using '{}' ({})",
            best.name, best.qid
        ))
    } else {
        None
    };

    Ok((Some(best.qid.clone()), warning))
}

pub fn gzip_bytes(input: &[u8]) -> std::io::Result<Vec<u8>> {
    use std::io::Write;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(input)?;
    encoder.finish()
}
