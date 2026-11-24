// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Structure, `InChIKey`, compound-name, and combined input resolution.

use lotus_model::{SearchCriteria, is_qid, looks_like_inchikey, normalize_structure};
use lotus_query::{
    CompoundMatch, compound_alias_query, compound_inchikey_query, compound_label_query,
    structure_compound_lookup_query,
};

use super::query_plan::ResolvedInputs;
use super::resolve_reference::resolve_reference;
use crate::client::Http;
use crate::error::{FetchError, ResponseFormat};

/// Resolve every input that is not already an identifier.
///
/// # Errors
/// Returns [`FetchError`] if a lookup cannot be run or read, and
/// [`FetchError::Parse`] if a name or a DOI matched nothing. A miss is a
/// validation failure to report, not a broken query: a name is a claim about
/// which compound is meant, so there is nothing to guess at when it matches
/// nothing.
///
/// Taxon is resolved by the caller, separately, because its result also carries
/// notes the caller prints. Structure and reference carry no such thing, so they
/// are one call.
pub async fn resolve_inputs<H: Http>(
    http: &H,
    criteria: &SearchCriteria,
) -> Result<ResolvedInputs, FetchError> {
    let mut resolved = ResolvedInputs::default();

    let structure = normalize_structure(&criteria.structure);
    if !structure.is_empty() {
        let trimmed = structure.trim();
        let found = resolve_structure(http, &structure).await?;
        if found.compounds.is_empty()
            && (looks_like_inchikey(trimmed) || lotus_model::could_be_a_compound_name(trimmed))
        {
            return Err(FetchError::Parse(format!(
                "no compound in Wikidata for '{}'",
                found.looked_up
            )));
        }
        if criteria.structure_search != lotus_model::SmilesSearchType::Exact && is_qid(trimmed) {
            let qid = trimmed.to_ascii_uppercase();
            let matches = lookup_compounds(http, &lotus_query::compound_by_qid_query(&qid)).await?;
            let Some(smiles) = first_canonical_smiles(&matches) else {
                return Err(FetchError::Parse(format!(
                    "no compound structure in Wikidata for '{qid}'"
                )));
            };
            resolved.smiles = Some(smiles);
        } else if found.smiles != trimmed {
            resolved.smiles = Some(found.smiles);
        }
        resolved.compounds = found.compounds;
    }

    resolved.reference_qid = resolve_reference(http, &criteria.reference).await?;
    Ok(resolved)
}

/// What a structure field resolved to.
///
/// Mirrors the client's resolution order exactly, because the two must agree: a
/// name typed into the CLI and the same name typed into the browser are the same
/// question, and answering them differently makes the CLI useless for reproducing
/// anything the app shows.
///
/// | Typed | Resolved by |
/// |---|---|
/// | a `QID` | passed through, nothing to look up |
/// | an `InChIKey` | `compound_inchikey_query` |
/// | a name | `compound_label_query`, then `compound_alias_query` |
/// | a SMILES or molfile | `structure_compound_lookup_query` (the service, cutoff 1) |
///
/// The asymmetry with taxa is deliberate and is the client's rule, not an
/// oversight: a name is a claim about *which* compound is meant, so a name that
/// matches nothing is an error, while a structure is not a claim at all -- a
/// SMILES Wikidata has no compound for is a perfectly good structure, and
/// searching it is what was asked for.
#[derive(Debug, Clone)]
pub struct StructureResolution {
    /// The compound QIDs a name, `InChIKey` or exact structure resolved to.
    ///
    /// More than one is normal: a structure at a cutoff of 1.0 matches
    /// stereoisomers and salts of the same thing, and `6d67d76` established that
    /// asking about only the first of those asks an arbitrary question.
    pub compounds: Vec<String>,
    /// The structure to search with: the reader's own input when nothing resolved,
    /// and the compound's canonical SMILES when a name identified one.
    pub smiles: String,
    /// The input as typed.
    pub looked_up: String,
}

/// Resolve a structure field to compound QIDs, in the client's order.
///
/// # Errors
/// Returns [`FetchError`] if a lookup cannot be run or read.
pub async fn resolve_structure<H: Http>(
    http: &H,
    input: &str,
) -> Result<StructureResolution, FetchError> {
    let normalized = normalize_structure(input);
    let trimmed = normalized.trim();
    let mut compounds = Vec::new();
    let mut canonical = None;

    if is_qid(trimmed) {
        compounds.push(trimmed.to_ascii_uppercase());
    } else if looks_like_inchikey(trimmed) {
        let by_key = lookup_compounds(http, &compound_inchikey_query(trimmed)).await?;
        canonical = first_canonical_smiles(&by_key);
        push_unique_qids(&mut compounds, by_key);
    } else if lotus_model::could_be_a_compound_name(trimmed) {
        let by_label = lookup_compounds(http, &compound_label_query(trimmed)).await?;
        canonical = first_canonical_smiles(&by_label);
        push_unique_qids(&mut compounds, by_label);
        if compounds.is_empty() {
            let by_alias = lookup_compounds(http, &compound_alias_query(trimmed)).await?;
            canonical = first_canonical_smiles(&by_alias);
            push_unique_qids(&mut compounds, by_alias);
        }
    } else {
        let by_structure =
            lookup_compounds(http, &structure_compound_lookup_query(&normalized)).await?;
        push_unique_qids(&mut compounds, by_structure);
    }

    Ok(StructureResolution {
        compounds,
        smiles: canonical.unwrap_or_else(|| normalized.clone()),
        looked_up: normalized,
    })
}

/// Append distinct compound QIDs in first-seen order.
fn push_unique_qids(target: &mut Vec<String>, matches: Vec<CompoundMatch>) {
    for found in matches {
        if !target.contains(&found.qid) {
            target.push(found.qid);
        }
    }
}

/// The first canonical SMILES a compound lookup found, skipping rows without one.
fn first_canonical_smiles(found: &[CompoundMatch]) -> Option<String> {
    found
        .iter()
        .map(|c| &c.canonical_smiles)
        .find(|s| !s.is_empty())
        .cloned()
}

/// Run a compound-lookup query and parse its rows.
///
/// # Errors
/// Returns [`FetchError`] if the lookup cannot be run or read.
async fn lookup_compounds<H: Http>(
    http: &H,
    query: &str,
) -> Result<Vec<CompoundMatch>, FetchError> {
    let csv = crate::execute_with_fallback(http, query, ResponseFormat::Csv)
        .await?
        .text()?;
    lotus_query::parse_compound_lookup_csv(csv.as_bytes())
        .map_err(|e| FetchError::Parse(e.to_string()))
}
