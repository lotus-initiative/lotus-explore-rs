// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
use crate::features::curation::services::execute_sparql_with_wdqs_fallback;
use lotus_curation::{
    CurationError, WD_TAXON_QID, WikidataCompound, build_compound_lookup_query,
    build_occurrence_query, build_occurrence_with_ref_query, build_reference_lookup_query,
    build_single_taxon_lookup_query, build_taxon_lookup_query, escape_quickstatements,
    extract_canonical_taxon_name, normalize_taxon_lookup, qid_from_uri as extract_qid_from_uri,
    qid_number, taxon_name_candidates,
};
use lotus_model::normalize_doi;
use lotus_search::{FetchError, ResponseFormat};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

fn binding_str<'a>(binding: &'a Value, key: &str) -> Option<&'a str> {
    binding
        .get(key)
        .and_then(|v| v.get("value"))
        .and_then(Value::as_str)
}

fn binding_value(binding: &Value, key: &str) -> Option<String> {
    binding_str(binding, key).map(ToOwned::to_owned)
}

fn binding_qid<'a>(binding: &'a Value, key: &str) -> Option<&'a str> {
    binding_str(binding, key).and_then(extract_qid_from_uri)
}

async fn execute_sparql_format(query: &str, format: ResponseFormat) -> Result<String, FetchError> {
    execute_sparql_with_wdqs_fallback(query, format).await
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

fn insert_lowest_by_qid<T>(
    map: &mut HashMap<String, T>,
    key: String,
    value: T,
    qid_of_item: impl Fn(&T) -> &str,
) {
    match map.entry(key) {
        std::collections::hash_map::Entry::Vacant(slot) => {
            slot.insert(value);
        }
        std::collections::hash_map::Entry::Occupied(mut slot) => {
            if qid_number(qid_of_item(&value)) < qid_number(qid_of_item(slot.get())) {
                slot.insert(value);
            }
        }
    }
}

fn choose_lowest_qid(map: &mut HashMap<String, String>, key: String, qid: &str) {
    insert_lowest_by_qid(map, key, qid.to_string(), |s| s.as_str());
}

fn extract_lowest_qid_from_json(json: &Value, var_name: &str) -> Option<String> {
    json_bindings(json)
        .filter_map(|binding| binding_qid(binding, var_name))
        .min_by_key(|qid| qid_number(qid))
        .map(str::to_owned)
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

    let mut resolved = HashMap::new();
    for chunk in keys.chunks(OCCURRENCE_CHUNK) {
        let json = execute_sparql_json(&build_compound_lookup_query(chunk)).await?;
        for (k, v) in compounds_by_key(&json) {
            insert_lowest_by_qid(&mut resolved, k, v, |c| &c.qid);
        }
    }
    Ok(resolved)
}

/// One item per `InChIKey`, from the lookup's bindings.
///
/// Several items can share a key. The lowest QID -- by number, so `Q20` comes
/// before `Q100` -- is kept, so the answer does not depend on the endpoint's
/// row order, and the properties kept are that item's own. Keeping the first
/// row's properties under a later row's QID judged one item by another's
/// statements: a missing `P233` went unreported, or a second, conflicting one
/// was proposed.
fn compounds_by_key(json: &Value) -> HashMap<String, WikidataCompound> {
    let mut resolved: HashMap<String, WikidataCompound> = HashMap::new();
    for binding in json_bindings(json) {
        let (Some(key), Some(qid)) = (
            binding_value(binding, "key"),
            binding_qid(binding, "compound"),
        ) else {
            continue;
        };
        let candidate = WikidataCompound {
            qid: qid.into(),
            canonical_smiles: binding_value(binding, "canonical"),
            isomeric_smiles: binding_value(binding, "iso"),
            inchi: binding_value(binding, "inchi"),
            formula: binding_value(binding, "formula"),
            mass: binding_str(binding, "mass").and_then(|v| v.parse::<f64>().ok()),
        };
        insert_lowest_by_qid(&mut resolved, key, candidate, |c| &c.qid);
    }
    resolved
}

/// The number in a QID, for ordering; a malformed one sorts last.
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

async fn query_existing(
    pairs: &[(String, String)],
) -> Result<HashSet<(String, String)>, CurationError> {
    if pairs.is_empty() {
        return Ok(HashSet::new());
    }
    let json = execute_sparql_json(&build_occurrence_query(pairs)).await?;
    Ok(json_bindings(&json)
        .filter_map(|binding| {
            Some((
                binding_qid(binding, "compound")?.to_string(),
                binding_qid(binding, "taxon")?.to_string(),
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
                binding_qid(binding, "compound")?.to_string(),
                binding_qid(binding, "taxon")?.to_string(),
                binding_qid(binding, "ref")?.to_string(),
            ))
        })
        .collect())
}

#[cfg(test)]
fn qid_of(value: &str) -> Option<String> {
    extract_qid_from_uri(value).map(str::to_string)
}

/// Returns `(Option<QID>, Vec<creation_QS_lines>)`.
/// If the taxon exists, returns `(Some(qid), [])`. Otherwise, returns `(None, minimal_CREATE_QS)`.
pub async fn resolve_or_create_taxon(
    name: &str,
    pre_resolved_qid: Option<&str>,
) -> Result<(Option<String>, Vec<String>), CurationError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Ok((None, Vec::new()));
    }
    if let Some(qid) = pre_resolved_qid {
        return Ok((Some(qid.into()), Vec::new()));
    }

    let Some(canonical) = extract_canonical_taxon_name(trimmed) else {
        return Ok((None, Vec::new()));
    };

    if let Some(qid) = resolve_taxon_qid(trimmed).await? {
        return Ok((Some(qid), Vec::new()));
    }
    let qs = vec![
        "## -- Step: create missing taxon --".into(),
        "CREATE".into(),
        format!("LAST|Len|\"{}\"", escape_quickstatements(&canonical)),
        format!("LAST|P31|{WD_TAXON_QID}"),
        format!("LAST|P225|\"{}\"", escape_quickstatements(&canonical)),
    ];
    Ok((None, qs))
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

    let mut resolved = HashMap::new();
    for chunk in normalized_dois.chunks(OCCURRENCE_CHUNK) {
        let query = build_reference_lookup_query(chunk);
        let json = execute_sparql_json(&query).await?;
        for (lookup, qid) in qids_by_lookup(&json, "ref") {
            choose_lowest_qid(&mut resolved, lookup, &qid);
        }
    }

    Ok(resolved)
}

fn qids_by_lookup(json: &Value, qid_var: &str) -> HashMap<String, String> {
    let mut resolved = HashMap::with_capacity(json_bindings_len(json));
    for binding in json_bindings(json) {
        let (Some(lookup), Some(qid)) = (
            binding_value(binding, "lookup"),
            binding_qid(binding, qid_var),
        ) else {
            continue;
        };
        choose_lowest_qid(&mut resolved, lookup, qid);
    }
    resolved
}

#[cfg(test)]
fn reference_qids_from_json(json: &Value) -> HashMap<String, String> {
    qids_by_lookup(json, "ref")
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

    let mut resolved = HashMap::new();
    for chunk in lookups.chunks(OCCURRENCE_CHUNK) {
        let query = build_taxon_lookup_query(chunk);
        let json = execute_sparql_json(&query).await?;
        for (lookup, qid) in qids_by_lookup(&json, "taxon") {
            choose_lowest_qid(&mut resolved, lookup, &qid);
        }
    }

    Ok(resolved)
}

#[cfg(test)]
fn taxon_qids_from_json(json: &Value) -> HashMap<String, String> {
    qids_by_lookup(json, "taxon")
}

pub(super) async fn resolve_taxon_qid(name: &str) -> Result<Option<String>, CurationError> {
    let Some(query) = build_single_taxon_lookup_query(name) else {
        return Ok(None);
    };
    let json = execute_sparql_json(&query).await?;
    Ok(extract_lowest_qid_from_json(&json, "taxon"))
}

fn parse_ask_boolean(parsed: &Value) -> Result<bool, CurationError> {
    parsed
        .get("boolean")
        .and_then(Value::as_bool)
        .ok_or_else(|| CurationError::Parse("SPARQL ASK response missing boolean result".into()))
}

pub async fn compound_has_taxon_with_ref(
    compound_qid: &str,
    taxon_qid: &str,
    ref_qid: &str,
) -> Result<bool, CurationError> {
    let query = lotus_curation::has_occurrence_with_ref_query(compound_qid, taxon_qid, ref_qid);
    let parsed = execute_sparql_json(&query).await?;
    parse_ask_boolean(&parsed)
}

pub async fn compound_has_taxon(
    compound_qid: &str,
    taxon_qid: &str,
) -> Result<bool, CurationError> {
    let query = lotus_curation::has_occurrence_query(compound_qid, taxon_qid);
    let parsed = execute_sparql_json(&query).await?;
    parse_ask_boolean(&parsed)
}

#[cfg(test)]
#[path = "wikidata/tests.rs"]
mod tests;
