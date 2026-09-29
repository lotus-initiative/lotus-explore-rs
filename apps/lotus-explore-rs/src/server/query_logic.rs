// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! SPARQL query construction, taxon resolution, and request transformation logic.
//! This module bridges the HTTP request layer and the upstream SPARQL endpoint:
//! More detail in the type and function docs below.

use crate::server::errors::ApiError;
use crate::server::state::{AppState, taxon_cache_get, taxon_cache_put};
use crate::server::types::SearchRequest;
use flate2::{Compression, write::GzEncoder};
use lotus::models::{SearchCriteria, TaxonMatch};
use lotus::{queries, sparql};

pub fn apply_request(req: &SearchRequest) -> Result<SearchCriteria, ApiError> {
    let mut c = SearchCriteria {
        taxon: req.taxon.clone().unwrap_or_default(),
        smiles: req.smiles.clone().unwrap_or_default(),
        ..Default::default()
    };

    if let Some(v) = req.smiles_search_type {
        c.smiles_search_type = v.into();
    }
    if let Some(v) = req.smiles_threshold {
        if v <= 0.0 {
            return Err(ApiError::bad_request(
                "smiles_threshold must be greater than 0",
            ));
        }
        c.smiles_threshold = v.clamp(0.05, 1.0);
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

pub fn build_execution_query(
    criteria: &SearchCriteria,
    resolved_taxon_qid: Option<&str>,
) -> String {
    let smiles = normalized_structure_input(&criteria.smiles);
    let base_query = if smiles.is_empty() {
        match resolved_taxon_qid {
            Some("*") | None => queries::query_all_compounds(),
            Some(qid) => queries::query_compounds_by_taxon(qid),
        }
    } else {
        let taxon_for_sachem = match resolved_taxon_qid {
            Some("*") => Some("Q2382443"),
            Some(qid) => Some(qid),
            None => None,
        };
        queries::query_sachem(
            &smiles,
            criteria.smiles_search_type,
            criteria.smiles_threshold,
            taxon_for_sachem,
        )
    };

    queries::query_with_server_filters(&base_query, criteria)
}

pub fn normalized_structure_input(value: &str) -> String {
    let normalized = if value.contains('\r') {
        value.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        value.to_string()
    };
    match queries::classify_structure(&normalized) {
        queries::StructureKind::MolfileV2000 | queries::StructureKind::MolfileV3000 => normalized,
        _ => normalized.trim().to_string(),
    }
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
    if is_qid(taxon) {
        return Ok((Some(taxon.to_ascii_uppercase()), None));
    }

    let sanitized = sanitize_taxon_input(taxon);
    let query = queries::query_taxon_search(&sanitized);
    let csv = sparql::execute_sparql_bytes(&query)
        .await
        .map_err(|e| ApiError::upstream(format!("taxon lookup failed: {e}")))?;
    let matches = sparql::parse_taxon_csv_bytes(&csv)
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

fn sanitize_taxon_input(taxon: &str) -> String {
    let replaced = taxon.replace('_', " ");
    let mut parts = replaced.split_whitespace();
    let Some(first_word) = parts.next() else {
        return replaced;
    };
    let mut chars = first_word.chars();
    let mut out = chars.next().map_or_else(String::new, |c| {
        c.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
    });
    for part in parts {
        out.push(' ');
        out.push_str(part);
    }
    out
}

fn is_qid(value: &str) -> bool {
    let v = value.trim();
    let mut chars = v.chars();
    matches!(chars.next(), Some('Q' | 'q')) && !v.is_empty() && chars.all(|c| c.is_ascii_digit())
}

pub fn gzip_bytes(input: &[u8]) -> std::io::Result<Vec<u8>> {
    use std::io::Write;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(input)?;
    encoder.finish()
}
