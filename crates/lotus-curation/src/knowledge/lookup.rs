// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Live SPARQL lookups against Wikidata for compound, taxon, reference, and occurrence existence.

use lotus_search::{FetchError, Http, ResponseFormat};

use crate::{
    CurationError, CurationInputRow, WikidataCompound, compound_by_inchikey_query,
    has_occurrence_query, is_binomial, qid_from_uri, reference_by_doi_query, taxon_by_name_query,
};

/// How a compound's structure is identified on Wikidata (`P235` `InChIKey`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StructureKey {
    /// The `InChIKey`, when the caller could compute one.
    pub inchikey: Option<String>,
}

impl StructureKey {
    /// Whether there is anything to look the compound up by.
    #[must_use]
    pub fn is_known(&self) -> bool {
        self.inchikey
            .as_deref()
            .is_some_and(|key| !key.trim().is_empty())
    }
}

/// What Wikidata holds for a compound, as a curation lookup sees it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WikidataLookup {
    /// The item, if there is one.
    pub compound: Option<WikidataCompound>,
    /// The taxon, if the row named one and Wikidata has it.
    pub taxon_qid: Option<String>,
    /// The reference, if the row named one and Wikidata has it.
    pub reference_qid: Option<String>,
    /// Whether the compound already records an occurrence in the taxon.
    pub has_occurrence: Option<bool>,
}

pub(super) fn first_bindings(
    json: &serde_json::Value,
) -> Option<&serde_json::Map<String, serde_json::Value>> {
    json.get("results")?
        .get("bindings")?
        .as_array()?
        .first()?
        .as_object()
}

fn binding<'a>(
    bindings: &'a serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Option<&'a str> {
    bindings
        .get(name)
        .and_then(|b| b.get("value"))
        .and_then(serde_json::Value::as_str)
}

pub(super) fn binding_qid(
    bindings: &serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Option<String> {
    binding(bindings, name)
        .and_then(qid_from_uri)
        .map(std::borrow::ToOwned::to_owned)
}

/// Look a row up, and say what is missing.
///
/// # Errors
/// Propagates any transport, status or decode failure.
pub async fn look_up<H: Http>(
    http: &H,
    row: &CurationInputRow,
    converted: &crate::ConvertedStructure,
) -> Result<WikidataLookup, CurationError> {
    let mut out = WikidataLookup::default();
    let key = converted.structure_key();

    if key.is_known() {
        let query = compound_by_inchikey_query(key.inchikey.as_deref().unwrap_or_default());
        let json = execute(http, &query).await?;
        if let Some(bindings) = first_bindings(&json) {
            out.compound = Some(WikidataCompound {
                qid: binding_qid(bindings, "compound").ok_or_else(|| {
                    CurationError::Parse("the compound came back without a QID".into())
                })?,
                canonical_smiles: binding(bindings, "canonical")
                    .map(std::borrow::ToOwned::to_owned),
                isomeric_smiles: binding(bindings, "iso").map(std::borrow::ToOwned::to_owned),
                inchi: binding(bindings, "inchi").map(std::borrow::ToOwned::to_owned),
                formula: binding(bindings, "formula").map(std::borrow::ToOwned::to_owned),
                mass: binding(bindings, "mass").and_then(|m| m.parse().ok()),
            });
        }
    }

    if let Some(taxon) = row
        .taxon
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        && is_binomial(taxon)
    {
        let json = execute(http, &taxon_by_name_query(taxon)).await?;
        if let Some(bindings) = first_bindings(&json) {
            out.taxon_qid = binding_qid(bindings, "taxon");
        }
    }

    if let Some(doi) = row.doi.as_deref().and_then(crate::normalize_doi) {
        let json = execute(http, &reference_by_doi_query(&doi)).await?;
        if let Some(bindings) = first_bindings(&json) {
            out.reference_qid = binding_qid(bindings, "ref");
        }
    }

    if let (Some(compound), Some(taxon)) = (&out.compound, &out.taxon_qid) {
        let query = out.reference_qid.as_deref().map_or_else(
            || has_occurrence_query(&compound.qid, taxon),
            |reference| crate::has_occurrence_with_ref_query(&compound.qid, taxon, reference),
        );
        let answer = ask(http, &query).await?;
        out.has_occurrence = Some(answer);
    }

    Ok(out)
}

async fn execute<H: Http>(http: &H, query: &str) -> Result<serde_json::Value, CurationError> {
    let answer = lotus_search::execute_with_fallback(http, query, ResponseFormat::SparqlJson)
        .await
        .map_err(|e: FetchError| CurationError::Http(e.to_string()))?;
    let text = answer
        .text()
        .map_err(|e: FetchError| CurationError::Parse(e.to_string()))?;
    serde_json::from_str(&text).map_err(|e| CurationError::Parse(e.to_string()))
}

pub(super) async fn ask<H: Http>(http: &H, query: &str) -> Result<bool, CurationError> {
    let value = execute(http, query).await?;
    value
        .as_bool()
        .or_else(|| value.get("boolean").and_then(serde_json::Value::as_bool))
        .ok_or_else(|| {
            CurationError::Parse(format!(
                "an ASK query did not return a boolean, but: {}",
                value.to_string().chars().take(120).collect::<String>()
            ))
        })
}
