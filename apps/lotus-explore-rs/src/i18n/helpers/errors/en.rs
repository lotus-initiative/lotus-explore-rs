// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

pub fn err_invalid_search_input() -> String {
    "Please enter a taxon name / QID, or a SMILES structure.".to_string()
}

pub fn err_api_not_configured() -> String {
    "LOTUS API is not configured.".to_string()
}

pub fn err_taxon_too_long() -> String {
    "Taxon input is too long. Please keep it under 500 characters.".to_string()
}

pub fn err_structure_too_long() -> String {
    "Structure input is too long. Please shorten the SMILES/Molfile text.".to_string()
}

pub fn err_mass_out_of_range() -> String {
    "Mass values must be between 0 and 10000.".to_string()
}

pub fn err_mass_range_invalid() -> String {
    "Mass minimum cannot exceed mass maximum.".to_string()
}

pub fn err_year_out_of_range() -> String {
    "Year is outside the supported range.".to_string()
}

pub fn err_year_range_invalid() -> String {
    "Year from cannot exceed year to.".to_string()
}

pub fn err_element_count_too_high() -> String {
    "Formula element counts are too high.".to_string()
}

pub fn err_similarity_threshold_invalid() -> String {
    "Similarity threshold must be greater than 0.".to_string()
}

pub fn err_unsupported_format(fmt: &str) -> String {
    format!("Unsupported format '{fmt}'. Use csv, json, or rdf.")
}

pub fn err_taxon_parse_failed(detail: &str) -> String {
    format!("Taxon parse failed: {detail}")
}

pub fn err_query_stage_failed(stage: &str, detail: &str) -> String {
    format!("{stage} failed: {detail}")
}

pub fn err_compound_not_found(input: &str) -> String {
    format!("Compound '{input}' not found in Wikidata.")
}

pub fn err_taxon_not_found(taxon: &str) -> String {
    format!("Taxon '{taxon}' not found in Wikidata.")
}

pub fn warn_input_standardized(original: &str, normalized: &str) -> String {
    format!("Input standardized from '{original}' to '{normalized}'.")
}

/// The common-name notice. Says which property matched, because that is
/// the actionable part: P1843 rather than P225.
pub fn warn_taxon_common_name(name: &str, qid: &str) -> String {
    format!(
        "Searching by common name is discouraged: '{name}' matched taxon {qid} by its common name (P1843) rather than its scientific name (P225)."
    )
}

/// The structure-resolved notice. Says which compound the name became,
/// because that is the actionable part: the search now runs against a
/// different structure than the one that was typed.
pub fn warn_compound_resolved(label: &str, qid: &str) -> String {
    format!("Resolved '{label}' to compound {qid}, by InChIKey, label or alias.")
}

pub fn warn_ambiguous_taxon(best_name: &str, best_qid: &str, names: &str) -> String {
    format!("Ambiguous taxon name; using {best_name} ({best_qid}). Candidates: {names}")
}

pub fn warn_wdqs_fallback() -> String {
    "Query executed via Wikidata Query Service (QLever fallback).".to_string()
}

#[cfg(target_arch = "wasm32")]
pub fn error_hint_memory() -> &'static str {
    "Result too large for current device memory."
}
