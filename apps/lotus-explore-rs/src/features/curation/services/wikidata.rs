// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
use super::helpers::{
    binding_value, escape_qs_string, escape_sparql_string, extract_qid_from_uri, normalize_doi,
};
use super::{
    CURATION_SPARQL_PREFIXES, CurationError, WD_OCCURS_IN_TAXON_PROP, WD_TAXON_QID,
    WikidataCompound,
};
use crate::sparql::{FetchError, ResponseFormat};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

async fn execute_sparql_format(query: &str, format: ResponseFormat) -> Result<String, FetchError> {
    super::execute_sparql_with_wdqs_fallback(query, format).await
}

async fn execute_sparql_json(query: &str) -> Result<Value, CurationError> {
    let raw = execute_sparql_format(query, ResponseFormat::SparqlJson)
        .await
        .map_err(|e| CurationError::Http(e.to_string()))?;
    serde_json::from_str::<Value>(&raw).map_err(|e| CurationError::Parse(e.to_string()))
}

fn json_bindings(json: &Value) -> impl Iterator<Item = &Value> {
    json.get("results")
        .and_then(|v| v.get("bindings"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

fn json_bindings_len(json: &Value) -> usize {
    json.get("results")
        .and_then(|v| v.get("bindings"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

fn first_json_binding(json: &Value) -> Option<&Value> {
    json.get("results")
        .and_then(|v| v.get("bindings"))
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
}

fn extract_first_qid_from_json(json: &Value, var_name: &str) -> Option<String> {
    first_json_binding(json)
        .and_then(|binding| binding.get(var_name))
        .and_then(|v| v.get("value"))
        .and_then(Value::as_str)
        .and_then(extract_qid_from_uri)
        .map(str::to_owned)
}

pub async fn fetch_wikidata_compound_by_inchikey(
    inchikey: &str,
) -> Result<Option<WikidataCompound>, CurationError> {
    let query = format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT ?compound ?canonical ?iso ?inchi ?formula ?mass WHERE {{\n  \
           ?compound wdt:P235 \"{}\" .\n  \
           OPTIONAL {{ ?compound wdt:P233 ?canonical . }}\n  \
           OPTIONAL {{ ?compound wdt:P2017 ?iso . }}\n  \
           OPTIONAL {{ ?compound wdt:P234 ?inchi . }}\n  \
           OPTIONAL {{ ?compound wdt:P274 ?formula . }}\n  \
           OPTIONAL {{ ?compound wdt:P2067 ?mass . }}\n\
         }} LIMIT 1",
        escape_sparql_string(inchikey)
    );
    let json = execute_sparql_json(&query).await?;
    let Some(first) = first_json_binding(&json) else {
        return Ok(None);
    };

    let qid = first
        .get("compound")
        .and_then(|v| v.get("value"))
        .and_then(Value::as_str)
        .and_then(extract_qid_from_uri)
        .ok_or_else(|| CurationError::Parse("missing compound qid".into()))?;

    Ok(Some(WikidataCompound {
        qid: qid.into(),
        canonical_smiles: binding_value(first, "canonical"),
        isomeric_smiles: binding_value(first, "iso"),
        inchi: binding_value(first, "inchi"),
        formula: binding_value(first, "formula"),
        mass: first
            .get("mass")
            .and_then(|v| v.get("value"))
            .and_then(Value::as_str)
            .and_then(|v| v.parse::<f64>().ok()),
    }))
}

/// Fetch every compound in a run with **one** query, keyed by the `InChIKey` the
/// caller computed locally, returning a map.
///
/// A `SELECT` with `VALUES`, not `N` `ASK`s or a join over every compound: one
/// index probe per key, same rows the per-key query returns. One POST per row
/// against a shared public endpoint is what made an import look like an attack.
/// A row absent from the map is absent from Wikidata.
///
/// No `LIMIT`: Wikidata has a handful of items for some structures, and that
/// ambiguity is per key, so `MIN(?compound)` picks deterministically instead of
/// letting plan order decide. The aggregate projects as **`?compound_item`** —
/// an `AS` clause may not target a variable the body already binds, and the
/// endpoint's refusal ("The target ?compound of an AS clause was already used in
/// the query body") is a `400` naming neither this function nor the run.
///
/// A function so a test can see the string that was sent.
fn build_compound_lookup_query(keys: &[String]) -> String {
    let values = inchi_values(keys);
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT ?key (MIN(?compound) AS ?compound_item) ?canonical ?iso ?inchi ?formula ?mass WHERE {{\n  \
           VALUES ?key {{ {values} }}\n  \
           ?compound wdt:P235 ?key .\n  \
           OPTIONAL {{ ?compound wdt:P233 ?canonical . }}\n  \
           OPTIONAL {{ ?compound wdt:P2017 ?iso . }}\n  \
           OPTIONAL {{ ?compound wdt:P234 ?inchi . }}\n  \
           OPTIONAL {{ ?compound wdt:P274 ?formula . }}\n  \
           OPTIONAL {{ ?compound wdt:P2067 ?mass . }}\n  \
         }} GROUP BY ?key ?canonical ?iso ?inchi ?formula ?mass"
    )
}

pub async fn fetch_compounds_by_inchikeys<'a>(
    inchikeys: impl IntoIterator<Item = &'a str>,
) -> Result<HashMap<String, WikidataCompound>, CurationError> {
    let mut keys: Vec<String> = Vec::new();
    let mut seen = HashSet::new();
    for key in inchikeys {
        let trimmed = key.trim();
        if !trimmed.is_empty() && seen.insert(trimmed.to_string()) {
            keys.push(trimmed.to_string());
        }
    }
    if keys.is_empty() {
        return Ok(HashMap::new());
    }

    let json = execute_sparql_json(&build_compound_lookup_query(&keys)).await?;

    let mut resolved: HashMap<String, WikidataCompound> = HashMap::new();
    for binding in json_bindings(&json) {
        // `compound_item`, not `compound`: the aggregate needs a target the body
        // did not already bind, and QLever answers `400 Invalid SPARQL query: The
        // target ?compound of an AS clause was already used in the query body`.
        let (Some(key), Some(qid)) = (
            binding_value(binding, "key"),
            binding
                .get("compound_item")
                .and_then(|v| v.get("value"))
                .and_then(Value::as_str)
                .and_then(extract_qid_from_uri),
        ) else {
            continue;
        };
        resolved
            .entry(key)
            .and_modify(|existing| {
                // Several candidate items for one key: keep the lowest QID, so the
                // answer does not depend on the endpoint's row order.
                if qid < existing.qid.as_str() {
                    existing.qid = qid.into();
                }
            })
            .or_insert_with(|| WikidataCompound {
                qid: qid.into(),
                canonical_smiles: binding_value(binding, "canonical"),
                isomeric_smiles: binding_value(binding, "iso"),
                inchi: binding_value(binding, "inchi"),
                formula: binding_value(binding, "formula"),
                mass: binding
                    .get("mass")
                    .and_then(|v| v.get("value"))
                    .and_then(Value::as_str)
                    .and_then(|v| v.parse::<f64>().ok()),
            });
    }
    Ok(resolved)
}

fn inchi_values(keys: &[String]) -> String {
    let mut values = String::with_capacity(keys.len() * 40);
    for (i, key) in keys.iter().enumerate() {
        if i > 0 {
            values.push(' ');
        }
        values.push('"');
        values.push_str(&escape_sparql_string(key));
        values.push('"');
    }
    values
}

/// Resolve every `(compound, taxon)` occurrence question in a run with **one**
/// query, returning which pairs Wikidata already records.
///
/// `N` per-pair `ASK`s are `N` requests to learn `N` bits, since an `ASK` returns
/// one boolean for the whole query. Asking for the pairs that *exist* returns the
/// same bits as the complement of one row set.
///
/// Chunked because one `VALUES` block with a thousand entries is a query the
/// endpoint must plan, and a planning failure fails the run. The caller picks the
/// chunk size; the default here exceeds what a spreadsheet of compounds produces
/// and stays well below what is expensive to plan.
pub async fn existing_occurrences(
    pairs: &[(String, String)],
    chunk_size: usize,
) -> Result<HashSet<(String, String)>, CurationError> {
    let mut existing = HashSet::new();
    for chunk in pairs.chunks(chunk_size.max(1)) {
        existing.extend(query_existing(chunk).await?);
    }
    Ok(existing)
}

/// The same for `(compound, taxon, reference)`, which is a different question
/// with a different answer: "reported by *this* paper", not "reported at all".
pub async fn existing_occurrences_with_ref(
    triples: &[(String, String, String)],
    chunk_size: usize,
) -> Result<HashSet<(String, String, String)>, CurationError> {
    let mut existing = HashSet::new();
    for chunk in triples.chunks(chunk_size.max(1)) {
        let rows = query_existing_with_ref(chunk).await?;
        existing.extend(rows);
    }
    Ok(existing)
}

/// The batch size used when a caller has no opinion.
pub const OCCURRENCE_CHUNK: usize = 100;

/// The batched "(compound, taxon) is recorded?" query, as a function so a test
/// can check it.
fn build_occurrence_query(pairs: &[(String, String)]) -> String {
    let values = entity_pair_values(pairs.iter().map(|(compound, taxon)| (compound, taxon)));
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT DISTINCT ?compound ?taxon WHERE {{\n  \
           VALUES (?compound ?taxon) {{ {values} }}\n  \
           ?compound wdt:{WD_OCCURS_IN_TAXON_PROP} ?taxon .\n\
         }}",
    )
}

/// The batched "(compound, taxon, reference) is recorded?" query.
fn build_occurrence_with_ref_query(triples: &[(String, String, String)]) -> String {
    let values = entity_triple_values(triples);
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT DISTINCT ?compound ?taxon ?ref WHERE {{\n  \
           VALUES (?compound ?taxon ?ref) {{ {values} }}\n  \
           ?compound wdt:{WD_OCCURS_IN_TAXON_PROP} ?taxon ;\n        \
                   wdt:P248 ?ref .\n\
         }}",
    )
}

fn entity_triple_values(triples: &[(String, String, String)]) -> String {
    let mut values = String::with_capacity(triples.len() * 64);
    for (i, (compound, taxon, reference)) in triples.iter().enumerate() {
        if i > 0 {
            values.push(' ');
        }
        values.push_str("(wd:");
        values.push_str(compound);
        values.push_str(" wd:");
        values.push_str(taxon);
        values.push_str(" wd:");
        values.push_str(reference);
        values.push(')');
    }
    values
}

async fn query_existing(
    pairs: &[(String, String)],
) -> Result<HashSet<(String, String)>, CurationError> {
    if pairs.is_empty() {
        return Ok(HashSet::new());
    }
    let json = execute_sparql_json(&build_occurrence_query(pairs)).await?;
    Ok(json_bindings(&json)
        .filter_map(|binding| {
            let compound = binding_value(binding, "compound")?;
            let taxon = binding_value(binding, "taxon")?;
            Some((
                extract_qid_from_uri(&compound)?.to_string(),
                extract_qid_from_uri(&taxon)?.to_string(),
            ))
        })
        .collect())
}

async fn query_existing_with_ref(
    triples: &[(String, String, String)],
) -> Result<HashSet<(String, String, String)>, CurationError> {
    if triples.is_empty() {
        return Ok(HashSet::new());
    }
    let json = execute_sparql_json(&build_occurrence_with_ref_query(triples)).await?;
    Ok(json_bindings(&json)
        .filter_map(|binding| {
            Some((
                qid_of(&binding_value(binding, "compound")?)?,
                qid_of(&binding_value(binding, "taxon")?)?,
                qid_of(&binding_value(binding, "ref")?)?,
            ))
        })
        .collect())
}

/// The QID out of a bound URI, owned.
///
/// `binding_value` hands back a `String`, so the `?` has to be applied to a
/// borrow of a value that outlives the expression -- which is the whole reason
/// this is a function and not an inline `and_then`.
fn qid_of(value: &str) -> Option<String> {
    extract_qid_from_uri(value).map(str::to_string)
}

fn entity_pair_values<'a>(pairs: impl Iterator<Item = (&'a String, &'a String)>) -> String {
    let mut values = String::new();
    for (i, (first, second)) in pairs.enumerate() {
        if i > 0 {
            values.push(' ');
        }
        values.push_str("(wd:");
        values.push_str(first);
        values.push_str(" wd:");
        values.push_str(second);
        values.push(')');
    }
    values
}

/// Returns `(Option<QID>, Vec<creation_QS_lines>)`.
/// If the taxon exists, returns `(Some(qid), [])`. Otherwise, returns `(None, minimal_CREATE_QS)`.
pub async fn resolve_or_create_taxon(
    name: &str,
    pre_resolved_qid: Option<&str>,
) -> Result<(Option<String>, Vec<String>), CurationError> {
    if let Some(qid) = pre_resolved_qid {
        return Ok((Some(qid.into()), Vec::new()));
    }

    if let Some(qid) = resolve_taxon_qid(name).await? {
        return Ok((Some(qid), Vec::new()));
    }
    let qs = vec![
        "## -- Step: create missing taxon --".into(),
        "CREATE".into(),
        format!("LAST|Len|\"{}\"", escape_qs_string(name)),
        format!("LAST|P31|{WD_TAXON_QID}"),
        format!("LAST|P225|\"{}\"", escape_qs_string(name)),
    ];
    Ok((None, qs))
}

pub(super) fn normalize_taxon_lookup(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_ascii_lowercase())
    }
}

fn canonicalize_taxon_label(name: &str) -> String {
    let mut words = name.split_whitespace();
    let Some(first) = words.next() else {
        return String::new();
    };

    // Pre-allocate with known length — output is same byte count as input.
    let mut rebuilt = String::with_capacity(name.len());
    let mut chars = first.chars();
    if let Some(ch) = chars.next() {
        rebuilt.push(ch.to_ascii_uppercase());
        rebuilt.extend(chars.map(|c| c.to_ascii_lowercase()));
    }
    for word in words {
        rebuilt.push(' ');
        rebuilt.push_str(&word.to_ascii_lowercase());
    }
    rebuilt
}

fn taxon_name_candidates(name: &str) -> Vec<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let canonical = canonicalize_taxon_label(trimmed);
    if canonical == trimmed {
        vec![trimmed.into()]
    } else {
        vec![trimmed.into(), canonical]
    }
}

fn build_single_taxon_lookup_query(name: &str) -> Option<String> {
    let candidates = taxon_name_candidates(name);
    if candidates.is_empty() {
        return None;
    }

    let mut values = String::with_capacity(candidates.len() * 40);
    for (i, candidate) in candidates.iter().enumerate() {
        if i > 0 {
            values.push(' ');
        }
        values.push('"');
        values.push_str(&escape_sparql_string(candidate));
        values.push('"');
    }

    Some(format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT ?taxon WHERE {{\n  \
           VALUES ?taxonName {{ {values} }}\n  \
           ?taxon wdt:P225 ?taxonName ;\n        \
                  wdt:P31 wd:Q16521 .\n\
         }} LIMIT 1"
    ))
}

fn build_reference_lookup_query(dois: &[String]) -> String {
    let mut values = String::with_capacity(dois.len() * 48);
    for (i, doi) in dois.iter().enumerate() {
        if i > 0 {
            values.push_str("\n    ");
        }
        let escaped = escape_sparql_string(doi);
        values.push_str("(\"");
        values.push_str(&escaped);
        values.push_str("\" \"");
        values.push_str(&escaped);
        values.push_str("\")");
    }

    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT ?lookup ?ref WHERE {{\n  \
           VALUES (?lookup ?doi) {{\n    \
             {values}\n  \
           }}\n  \
           ?ref wdt:P356 ?doi .\n\
         }}"
    )
}

pub async fn resolve_reference_qids_batch<'a>(
    dois: impl IntoIterator<Item = &'a str>,
) -> Result<HashMap<String, String>, CurationError> {
    let mut seen = HashSet::new();
    let mut normalized_dois = Vec::new();

    for doi in dois {
        let Some(normalized) = normalize_doi(doi) else {
            continue;
        };
        // Keep only first occurrence per normalized DOI.
        if seen.insert(normalized.clone()) {
            normalized_dois.push(normalized);
        }
    }

    if normalized_dois.is_empty() {
        return Ok(HashMap::new());
    }

    let query = build_reference_lookup_query(&normalized_dois);
    let json = execute_sparql_json(&query).await?;

    let mut resolved = HashMap::with_capacity(json_bindings_len(&json));
    for binding in json_bindings(&json) {
        let Some(lookup) = binding_value(binding, "lookup") else {
            continue;
        };
        let Some(qid) = binding
            .get("ref")
            .and_then(|v| v.get("value"))
            .and_then(Value::as_str)
            .and_then(extract_qid_from_uri)
        else {
            continue;
        };
        resolved.insert(lookup, qid.into());
    }

    Ok(resolved)
}

fn build_taxon_lookup_query(lookups: &[(String, String)]) -> String {
    let mut values = String::with_capacity(lookups.len() * 64);
    for (i, (lookup, taxon_name)) in lookups.iter().enumerate() {
        if i > 0 {
            values.push_str("\n    ");
        }
        values.push('(');
        values.push('"');
        values.push_str(&escape_sparql_string(lookup));
        values.push_str("\" \"");
        values.push_str(&escape_sparql_string(taxon_name));
        values.push_str("\")");
    }

    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT ?lookup ?taxon WHERE {{\n  \
           VALUES (?lookup ?taxonName) {{\n    \
             {values}\n  \
           }}\n  \
           ?taxon wdt:P225 ?taxonName ;\n        \
                  wdt:P31 wd:Q16521 .\n\
         }}"
    )
}

pub async fn resolve_taxon_qids_batch<'a>(
    names: impl IntoIterator<Item = &'a str>,
) -> Result<HashMap<String, String>, CurationError> {
    let mut lookups = Vec::new();
    let mut seen = HashSet::new();

    for name in names {
        let trimmed = name.trim();
        let Some(lookup) = normalize_taxon_lookup(trimmed) else {
            continue;
        };
        if seen.insert(lookup.clone()) {
            // Both spellings, under one lookup key. The single-row lookup has
            // always tried the canonicalised label as well, and a batch that only
            // tried the raw one sent every such row down the per-row path to be
            // asked exactly the same question again -- one request per row, for an
            // answer the batch had already been in a position to get.
            for candidate in taxon_name_candidates(trimmed) {
                lookups.push((lookup.clone(), candidate));
            }
        }
    }

    if lookups.is_empty() {
        return Ok(HashMap::new());
    }

    let query = build_taxon_lookup_query(&lookups);
    let json = execute_sparql_json(&query).await?;

    let mut resolved = HashMap::with_capacity(json_bindings_len(&json));
    for binding in json_bindings(&json) {
        let Some(lookup) = binding_value(binding, "lookup") else {
            continue;
        };
        let Some(qid) = binding
            .get("taxon")
            .and_then(|v| v.get("value"))
            .and_then(Value::as_str)
            .and_then(extract_qid_from_uri)
        else {
            continue;
        };

        resolved.insert(lookup, qid.into());
    }

    Ok(resolved)
}

pub(super) async fn resolve_taxon_qid(name: &str) -> Result<Option<String>, CurationError> {
    let Some(query) = build_single_taxon_lookup_query(name) else {
        return Ok(None);
    };
    let json = execute_sparql_json(&query).await?;
    Ok(extract_first_qid_from_json(&json, "taxon"))
}

pub async fn resolve_reference_qid(doi: &str) -> Result<Option<String>, CurationError> {
    // Normalize DOI: strip the `doi.org/` prefix (case-insensitive) and uppercase.
    // Wikidata stores P356 (DOI) values in uppercase without the prefix.
    let normalized = normalize_doi(doi)
        .ok_or_else(|| CurationError::InvalidInput("invalid or empty DOI".to_string()))?;
    let query = format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT ?ref WHERE {{\n  \
           ?ref wdt:P356 \"{}\" .\n         \
         }} LIMIT 1",
        escape_sparql_string(&normalized)
    );
    let json = execute_sparql_json(&query).await?;
    Ok(extract_first_qid_from_json(&json, "ref"))
}

pub async fn compound_has_taxon_with_ref(
    compound_qid: &str,
    taxon_qid: &str,
    ref_qid: &str,
) -> Result<bool, CurationError> {
    let query = format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         ASK {{\n  \
           wd:{compound_qid} p:P703 ?stmt .\n  \
           ?stmt ps:P703 wd:{taxon_qid} ;\n        \
                 prov:wasDerivedFrom ?refnode .\n  \
           ?refnode pr:P248 wd:{ref_qid} .\n\
         }}"
    );
    let parsed = execute_sparql_json(&query).await?;
    Ok(parsed
        .get("boolean")
        .and_then(Value::as_bool)
        .unwrap_or(false))
}

pub async fn compound_has_taxon(
    compound_qid: &str,
    taxon_qid: &str,
) -> Result<bool, CurationError> {
    let query = format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         ASK {{ wd:{compound_qid} wdt:{WD_OCCURS_IN_TAXON_PROP} wd:{taxon_qid} . }}"
    );
    let parsed = execute_sparql_json(&query).await?;
    Ok(parsed
        .get("boolean")
        .and_then(Value::as_bool)
        .unwrap_or(false))
}

#[cfg(test)]
#[path = "wikidata/tests.rs"]
mod tests;
