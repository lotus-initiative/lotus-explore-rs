// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Counts, narrow occurrence-edge rewrites, and batched dictionary enrichment queries.

use std::fmt::Write as _;

use super::prefixes::{COMPOUND_PROPERTIES, PREFIXES, REFERENCE_METADATA, SELECT_CLAUSE};

/// The largest `VALUES` list sent in a single dictionary enrichment request.
///
/// Public `QLever` deployments enforce a 1 MB `client_max_body_size` at the reverse
/// proxy (`50,000` `wd:Q…` tokens is ~750 KB URL-encoded). Above this threshold,
/// compound enrichment switches to a single parameter-free LOTUS compound scan and
/// reference enrichment chunks `VALUES ?r` slices of at most this size.
pub const DICTIONARY_VALUES_LIMIT: usize = 50_000;

const OCCURRENCES_SELECT: &str = r#"
SELECT
  (xsd:integer(STRAFTER(STR(?c), "Q")) AS ?compound)
  (xsd:integer(STRAFTER(STR(?t), "Q")) AS ?taxon)
  ?taxon_name
  (xsd:integer(STRAFTER(STR(?r), "Q")) AS ?ref_qid)
"#;

/// Wrap a `SELECT` in a `COUNT`, keeping the filters but dropping everything the
/// count does not depend on.
///
/// The display `OPTIONAL`s are stripped, and that is the point: `rdfs:label` with two
/// `FILTER(LANG(…))` passes is the most expensive thing in the query, slow enough in a
/// count to be rate-limited. Variables left unbound project as `NULL`, which the `COUNT`s
/// ignore.
///
/// The deletion is by literal text, correct only because nothing a filter needs lives inside
/// the deleted blocks. [`with_filters`](super::with_filters) binds what a filter needs outside them: `?c
/// wdt:P2067 ?compound_mass .` for mass, `?r wdt:P577 ?ref_date .` for year, `?c
/// wdt:P274 ?compound_formula .` for formula. A filter reading a variable bound only inside a
/// deleted block survives as a `FILTER` over an unbound variable and counts nothing -- which
/// is what the formula filter did until its binding was added, and every count on the page
/// read zero for any search using one.
#[must_use]
pub fn counts_query(base: &str) -> String {
    let Some(select_at) = base.find("SELECT") else {
        return base.to_string();
    };
    let prefixes = &base[..select_at];
    let mut stripped = base[select_at..]
        .replace(REFERENCE_METADATA, "")
        .replace(COMPOUND_PROPERTIES, "");

    // The year filter constrains `?ref_date`, and the block stripped above is what binds
    // it. Left alone, the filter references an unbound variable, every solution is filtered
    // out, and every count reads zero -- silently, confidently wrong.
    //
    // Rebinding is not a workaround: the metadata OPTIONAL and this triple both supply
    // `?ref_date` from `?r wdt:P577`, and the filter's own `BOUND()` drops dateless rows
    // either way, so both queries count the same set.
    //
    // The triple goes immediately before the filter, inside the same group. Appended at
    // the end it would land after the base query's closing brace -- outside the filter's
    // scope, and beside a sub-select, which SPARQL does not allow.
    if stripped.contains("FILTER(BOUND(?ref_date)") {
        stripped = stripped.replacen(
            "FILTER(BOUND(?ref_date)",
            "?r wdt:P577 ?ref_date .\nFILTER(BOUND(?ref_date)",
            1,
        );
    }

    format!(
        "{prefixes}
SELECT
  (COUNT(*) AS ?n_entries)
  (COUNT(DISTINCT CONCAT(
    STR(?compound), \"\\u001F\", COALESCE(STR(?taxon), \"\"), \"\\u001F\", COALESCE(STR(?ref_qid), \"\")
  )) AS ?n_entries_unique)
  (COUNT(DISTINCT ?compound) AS ?n_compounds)
  (COUNT(DISTINCT ?taxon) AS ?n_taxa)
  (COUNT(DISTINCT ?ref_qid) AS ?n_references)
WHERE {{
  {{
    {}
  }}
}}",
        stripped.trim()
    )
}

/// Rewrite a full result `SELECT` into the normalized occurrence-edge query.
///
/// Strips `REFERENCE_METADATA` and `COMPOUND_PROPERTIES` (re-binding `?ref_date`
/// when a year filter is active) and projects only the numeric entity QIDs plus
/// `?taxon_name`. That avoids joining 9 `OPTIONAL` metadata properties across every
/// occurrence edge and avoids projecting `VocabularyOnDisk` statement and
/// reference-node URIs.
#[must_use]
pub fn occurrences_query(base: &str) -> String {
    if !base.contains(SELECT_CLAUSE) {
        return base.to_string();
    }
    base.replacen(SELECT_CLAUSE, OCCURRENCES_SELECT, 1)
        .replace(REFERENCE_METADATA, "")
        .replace(COMPOUND_PROPERTIES, "")
        .replace(
            "FILTER(BOUND(?ref_date)",
            "?r wdt:P577 ?ref_date .\nFILTER(BOUND(?ref_date)",
        )
}

fn format_values_clause(var: &str, qids: &[u32]) -> String {
    let mut out = format!("VALUES {var} {{");
    for &qid in qids {
        let _ = write!(out, " wd:Q{qid}");
    }
    out.push_str(" }");
    out
}

/// Dictionary enrichment query for the distinct compounds in an occurrence set.
#[must_use]
pub fn compound_dictionary_query(qids: &[u32]) -> String {
    let scope = if qids.len() > DICTIONARY_VALUES_LIMIT {
        "{\n    SELECT DISTINCT ?c ?compound_inchikey\n    WHERE {\n      ?c wdt:P235 ?compound_inchikey ;\n         p:P703 ?_s .\n      ?_s wikibase:rank ?_sr .\n      FILTER(?_sr != wikibase:DeprecatedRank)\n    }\n  }"
            .to_string()
    } else {
        format!(
            "{}\n  OPTIONAL {{ ?c wdt:P235 ?compound_inchikey . }}",
            format_values_clause("?c", qids)
        )
    };
    format!(
        "{PREFIXES}\n\
SELECT
  (xsd:integer(STRAFTER(STR(?c), \"Q\")) AS ?compound)
  (COALESCE(?compoundLabelMul, ?compoundLabelEn) AS ?compoundLabel)
  ?compound_inchikey
  (COALESCE(?_smiles_iso, ?compound_smiles_conn) AS ?compound_smiles_iso)
  ?compound_mass
  ?compound_formula
WHERE {{
  {scope}
  OPTIONAL {{ ?c wdt:P233 ?compound_smiles_conn . }}
  OPTIONAL {{ ?c wdt:P2017 ?_smiles_iso . }}
  OPTIONAL {{ ?c wdt:P2067 ?compound_mass . }}
  OPTIONAL {{ ?c wdt:P274 ?compound_formula . }}
  OPTIONAL {{ ?c rdfs:label ?compoundLabelEn . FILTER(LANG(?compoundLabelEn) = \"en\") }}
  OPTIONAL {{ ?c rdfs:label ?compoundLabelMul . FILTER(LANG(?compoundLabelMul) = \"mul\") }}
}}"
    )
}

/// Dictionary enrichment query for the distinct references in an occurrence set.
#[must_use]
pub fn reference_dictionary_query(qids: &[u32]) -> String {
    // An unconstrained `?_c p:P703/prov:wasDerivedFrom/pr:P248 ?r` property path
    // causes QLever to join all 1.33B `prov:wasDerivedFrom` triples in Wikidata
    // before intersecting with `p:P703`, timing out on `Sort on ?_QLever_internal_variable_qp_1`.
    // When `qids` exceeds the single-request `VALUES` limit, materializing
    // the LOTUS `p:P703` statements in an inner `SELECT ?_s` subquery forces QLever
    // to probe `prov:wasDerivedFrom` only for the ~2.99M LOTUS statement nodes.
    let scope = if qids.len() > DICTIONARY_VALUES_LIMIT {
        "{\n    SELECT DISTINCT ?r\n    WHERE {\n      {\n        SELECT ?_s\n        WHERE {\n          ?_c wdt:P235 ?_ik ;\n              p:P703 ?_s .\n          ?_s wikibase:rank ?_sr .\n          FILTER(?_sr != wikibase:DeprecatedRank)\n        }\n      }\n      ?_s prov:wasDerivedFrom ?_rn .\n      ?_rn pr:P248 ?r .\n    }\n  }"
            .to_string()
    } else {
        format_values_clause("?r", qids)
    };
    format!(
        "{PREFIXES}\n\
SELECT
  (xsd:integer(STRAFTER(STR(?r), \"Q\")) AS ?ref_qid)
  ?ref_title
  ?ref_doi
  (YEAR(?ref_date) AS ?ref_year)
WHERE {{
  {scope}
  OPTIONAL {{ ?r wdt:P1476 ?ref_title . }}
  OPTIONAL {{ ?r wdt:P356 ?ref_doi . }}
  OPTIONAL {{ ?r wdt:P577 ?ref_date . }}
}}"
    )
}

/// Combined dictionary enrichment query fetching both compound and reference
/// metadata in a single `UNION` request when their combined count fits within
/// [`DICTIONARY_VALUES_LIMIT`].
#[must_use]
pub fn entity_dictionary_query(compound_qids: &[u32], reference_qids: &[u32]) -> String {
    let compounds = format_values_clause("?c", compound_qids);
    let references = format_values_clause("?r", reference_qids);
    format!(
        "{PREFIXES}\n\
SELECT
  (xsd:integer(STRAFTER(STR(?c), \"Q\")) AS ?compound)
  (COALESCE(?compoundLabelMul, ?compoundLabelEn) AS ?compoundLabel)
  ?compound_inchikey
  (COALESCE(?_smiles_iso, ?compound_smiles_conn) AS ?compound_smiles_iso)
  ?compound_mass
  ?compound_formula
  (xsd:integer(STRAFTER(STR(?r), \"Q\")) AS ?ref_qid)
  ?ref_title
  ?ref_doi
  (YEAR(?ref_date) AS ?ref_year)
WHERE {{
  {{
    {compounds}
    OPTIONAL {{ ?c wdt:P235 ?compound_inchikey . }}
    OPTIONAL {{ ?c wdt:P233 ?compound_smiles_conn . }}
    OPTIONAL {{ ?c wdt:P2017 ?_smiles_iso . }}
    OPTIONAL {{ ?c wdt:P2067 ?compound_mass . }}
    OPTIONAL {{ ?c wdt:P274 ?compound_formula . }}
    OPTIONAL {{ ?c rdfs:label ?compoundLabelEn . FILTER(LANG(?compoundLabelEn) = \"en\") }}
    OPTIONAL {{ ?c rdfs:label ?compoundLabelMul . FILTER(LANG(?compoundLabelMul) = \"mul\") }}
  }}
  UNION
  {{
    {references}
    OPTIONAL {{ ?r wdt:P1476 ?ref_title . }}
    OPTIONAL {{ ?r wdt:P356 ?ref_doi . }}
    OPTIONAL {{ ?r wdt:P577 ?ref_date . }}
  }}
}}"
    )
}

/// Dictionary enrichment queries for `compound_qids` and `reference_qids`.
#[must_use]
pub fn dictionary_queries(compound_qids: &[u32], reference_qids: &[u32]) -> Vec<String> {
    if !compound_qids.is_empty()
        && !reference_qids.is_empty()
        && compound_qids.len().saturating_add(reference_qids.len()) <= DICTIONARY_VALUES_LIMIT
    {
        return vec![entity_dictionary_query(compound_qids, reference_qids)];
    }
    let mut list = Vec::new();
    if !compound_qids.is_empty() {
        list.push(compound_dictionary_query(compound_qids));
    }
    if !reference_qids.is_empty() {
        list.push(reference_dictionary_query(reference_qids));
    }
    list
}
