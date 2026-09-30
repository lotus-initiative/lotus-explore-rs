// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Curation against a live Wikidata.
//!
//! The queries are in `wikidata_query`; this runs them. The transport is the
//! [`Http`] trait from `lotus-search`, so the whole of curation can be tested
//! against a scripted conversation -- which matters more here than elsewhere,
//! because a curation run touches the network once per row per dependency, and
//! a test that hits Wikidata is a test that is skipped when the network is
//! down and wrong when the data changes.
//!
//! Nothing here submits. It reads what is there and reports the difference, and
//! the statements it produces are for a person to review.

use std::fmt::Write as _;

use lotus_search::{FetchError, Http, ResponseFormat};

use crate::{
    CurationError, CurationInputRow, CurationResultRow, CurationStatus, WikidataCompound,
    compound_by_inchikey_query, create_compound_statements, escape_quickstatements,
    has_occurrence_query, is_binomial, property, qid_from_uri, reference_by_doi_query,
    taxon_by_name_query,
};

/// How a compound's structure is identified on Wikidata.
///
/// `P235`, the `InChIKey`. Not the SMILES: a chemist can write the same molecule
/// several ways, and a curation run that matched on SMILES would call a
/// compound "new" when it had been there for years.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StructureKey {
    /// The `InChIKey`, when the caller could compute one.
    pub inchikey: Option<String>,
}

impl StructureKey {
    /// Whether there is anything to look the compound up by.
    ///
    /// Without an `InChIKey` the only honest answer is "cannot tell", and a
    /// curation run reports that rather than guessing from the SMILES.
    #[must_use]
    pub fn is_known(&self) -> bool {
        self.inchikey
            .as_deref()
            .is_some_and(|key| !key.trim().is_empty())
    }
}

/// Read a Wikidata item's bindings out of a SPARQL JSON result.
///
/// The shape is `results.bindings[0]`: `results` is an *object* that holds a
/// `bindings` array, not an array itself. Reading it as an array finds nothing,
/// and a lookup that finds nothing reads as "not in Wikidata" -- the one answer
/// that must never come from a parsing slip.
///
/// `None` means the pattern matched no item, which is the answer a curation run
/// most needs.
fn first_bindings(json: &serde_json::Value) -> Option<&serde_json::Map<String, serde_json::Value>> {
    json.get("results")?
        .get("bindings")?
        .as_array()?
        .first()?
        .as_object()
}

/// The value of one binding, as a string.
fn binding<'a>(
    bindings: &'a serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Option<&'a str> {
    bindings
        .get(name)
        .and_then(|b| b.get("value"))
        .and_then(serde_json::Value::as_str)
}

/// The QID of one binding, read out of a URI.
fn binding_qid(
    bindings: &serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Option<String> {
    binding(bindings, name)
        .and_then(qid_from_uri)
        .map(std::borrow::ToOwned::to_owned)
}

/// What Wikidata holds for a compound, as a `CurationKnowledge` sees it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WikidataLookup {
    /// The item, if there is one.
    pub compound: Option<WikidataCompound>,
    /// The taxon, if the row named one and Wikidata has it.
    pub taxon_qid: Option<String>,
    /// The reference, if the row named one and Wikidata has it.
    pub reference_qid: Option<String>,
    /// Whether the compound already records an occurrence in the taxon.
    ///
    /// `None` when there is nothing to check against -- no compound, or no taxon
    /// -- which is different from `Some(false)`: the first means the question
    /// was not asked, the second that the answer was no.
    pub has_occurrence: Option<bool>,
}

/// Look a row up, and say what is missing.
///
/// # Errors
/// Propagates any transport, status or decode failure. A row that cannot be
/// looked up is an error rather than a "new", because a network failure that
/// reads as "not in Wikidata" produces a duplicate submission every time it
/// happens.
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
    {
        // A genus on its own is ambiguous, and resolving it to an arbitrary
        // species would be a data error rather than a near miss. Reported, not
        // guessed.
        if is_binomial(taxon) {
            let json = execute(http, &taxon_by_name_query(taxon)).await?;
            if let Some(bindings) = first_bindings(&json) {
                out.taxon_qid = binding_qid(bindings, "taxon");
            }
        }
    }

    if let Some(doi) = row.doi.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        let json = execute(http, &reference_by_doi_query(doi)).await?;
        if let Some(bindings) = first_bindings(&json) {
            out.reference_qid = binding_qid(bindings, "ref");
        }
    }

    if let (Some(compound), Some(taxon)) = (&out.compound, &out.taxon_qid) {
        let answer = ask(http, &has_occurrence_query(&compound.qid, taxon)).await?;
        out.has_occurrence = Some(answer);
    }

    Ok(out)
}

/// Run a query and return the parsed answer.
async fn execute<H: Http>(http: &H, query: &str) -> Result<serde_json::Value, CurationError> {
    // With the WDQS fallback, because curation is a run over a whole file and a
    // single hard failure partway through is worse than a slower answer.
    let answer = lotus_search::execute_with_fallback(http, query, ResponseFormat::SparqlJson)
        .await
        .map_err(|e: FetchError| CurationError::Http(e.to_string()))?;
    let text = answer
        .text()
        .map_err(|e: FetchError| CurationError::Parse(e.to_string()))?;
    // `execute` already refuses an empty body, so reaching here means there are
    // bytes; a parse failure now is a shape this crate does not read.
    serde_json::from_str(&text).map_err(|e| CurationError::Parse(e.to_string()))
}

/// Run an `ASK`, which answers with a bare boolean rather than a result set.
async fn ask<H: Http>(http: &H, query: &str) -> Result<bool, CurationError> {
    let value = execute(http, query).await?;
    value
        .as_bool()
        .or_else(|| value.get("boolean").and_then(serde_json::Value::as_bool))
        .ok_or_else(|| {
            CurationError::Parse(format!(
                "an ASK query did not return a boolean, but: {}",
                // Truncated: a full result set for an accidental SELECT can be
                // megabytes, and the shape is what matters here.
                value.to_string().chars().take(120).collect::<String>()
            ))
        })
}

/// The statements a row needs, and what curation concluded about it.
///
/// Empty when the row is already correct, which is the whole point: the answer
/// to "what should I submit" is often "nothing".
#[must_use]
pub fn statements_for(
    row: &CurationInputRow,
    converted: &crate::ConvertedStructure,
    lookup: &WikidataLookup,
) -> Vec<String> {
    let mut out = Vec::new();
    let name = escape_quickstatements(row.name.trim());

    if lookup.compound.is_none() {
        if converted.is_known() {
            // Not in Wikidata, and the structure is known: this is the one case
            // where a `CREATE` is the right output. The class comes first, because
            // an item with no class is not findable by anything that filters on
            // one.
            let canonical = converted.canonical_smiles.as_deref().unwrap_or_default();
            let mut block = create_compound_statements(row.name.trim(), canonical, canonical);
            if let Some(inchikey) = converted.inchikey.as_deref() {
                let _ = writeln!(
                    block,
                    "\nLAST|{key}|\"{inchikey}\"",
                    key = property::INCHIKEY
                );
            }
            out.push(block);
        } else {
            // No item and no key: there is nothing to assert about a structure
            // nobody can identify, and a statement built from the raw SMILES
            // would create a second item for a molecule that already has one.
            out.push(format!(
                "## {name}\n# no InChIKey for this structure, so no item can be \
                 created or matched. Check the structure in this row."
            ));
        }
        return out;
    }

    if let (Some(compound), Some(taxon)) = (&lookup.compound, &lookup.taxon_qid)
        && lookup.has_occurrence == Some(false)
    {
        // The comment names the item, because a bundle of these is read by a
        // person months later and `wd:Q123` on its own does not say which of
        // forty rows it belonged to.
        out.push(format!(
            "## {name} — {qid} in {taxon}\nCREATE\n  LAST|{occurs}|wd:{taxon}",
            qid = compound.qid,
            occurs = property::OCCURS_IN_TAXON,
        ));
    }

    out
}

/// The statements that would create a taxon the row needs and Wikidata lacks.
///
/// These are dependencies, not the row's own statements: a `QuickStatements` run
/// stops at the first failure, so an occurrence pointing at an item that does not
/// exist takes the occurrence down with it, and the compound with that.
#[must_use]
pub fn taxon_dependency_statements(row: &CurationInputRow) -> Vec<String> {
    let Some(taxon) = row
        .taxon
        .as_deref()
        .map(str::trim)
        .filter(|taxon| !taxon.is_empty())
    else {
        return Vec::new();
    };
    // A genus on its own is ambiguous, so there is no statement that would be
    // right: creating "Gentiana" as a taxon would be creating a real item from
    // an ambiguous name.
    if !is_binomial(taxon) {
        return Vec::new();
    }
    vec![format!(
        "## {name} — taxon\nCREATE\n  LAST|{class}|wd:{taxon_class}\n  LAST|{name_prop}|\"{name}\"",
        name = escape_quickstatements(taxon),
        class = "P31",
        taxon_class = crate::WD_TAXON_QID,
        name_prop = property::SCIENTIFIC_NAME,
    )]
}

/// Turn a lookup into a result row.
#[must_use]
pub fn to_result_row(
    row: &CurationInputRow,
    converted: &crate::ConvertedStructure,
    lookup: &WikidataLookup,
) -> CurationResultRow {
    let key = converted.structure_key();
    let statements = statements_for(row, converted, lookup);

    // A taxon the row names but Wikidata does not have is a dependency that is
    // not there yet, not a finished row. Reporting it as complete is the worst
    // outcome available: the curator reads "nothing to do", submits nothing, and
    // the finding is silently lost.
    let taxon_named = row
        .taxon
        .as_deref()
        .map(str::trim)
        .is_some_and(|taxon| !taxon.is_empty());
    let taxon_pending = taxon_named && lookup.taxon_qid.is_none();

    let status = if !key.is_known() {
        // Not "new": the run could not tell, and reporting a compound as new
        // because the identifier was missing is how a duplicate gets made.
        CurationStatus::Error
    } else if lookup.compound.is_some() {
        if lookup.has_occurrence == Some(false) {
            CurationStatus::ExistingNeedsUpdates
        } else if taxon_pending {
            CurationStatus::PendingDependencies
        } else {
            // The compound is there, the occurrence is there or was never asked
            // for, and there is nothing outstanding.
            CurationStatus::ExistingComplete
        }
    } else if taxon_pending {
        CurationStatus::PendingDependencies
    } else {
        CurationStatus::NewCompound
    };

    let mut note = String::new();
    if !key.is_known() {
        note.push_str("no InChIKey for this structure; nothing could be looked up. ");
    }
    if let Some(taxon) = row.taxon.as_deref() {
        let taxon = taxon.trim();
        if !taxon.is_empty() && !is_binomial(taxon) {
            note.push_str("the taxon is a genus on its own, which is ambiguous. ");
        } else if lookup.taxon_qid.is_none() {
            note.push_str("Wikidata has no taxon by that name. ");
        }
    }
    if let Some(doi) = row.doi.as_deref()
        && !doi.trim().is_empty()
        && lookup.reference_qid.is_none()
    {
        note.push_str("Wikidata has no reference with that DOI. ");
    }

    CurationResultRow {
        input: row.clone(),
        // The curator's own structure is what was submitted; Wikidata's copy is
        // recorded in `input` terms above. A difference between the two is a
        // finding, so the submitted one is kept.
        canonical_smiles: converted.canonical_smiles.clone().or_else(|| {
            lookup
                .compound
                .as_ref()
                .and_then(|c| c.canonical_smiles.clone())
        }),
        inchikey: key.inchikey,
        inchi: lookup.compound.as_ref().and_then(|c| c.inchi.clone()),
        formula: lookup.compound.as_ref().and_then(|c| c.formula.clone()),
        exact_mass: lookup.compound.as_ref().and_then(|c| c.mass),
        mass_warning: None,
        wikidata_qid: lookup.compound.as_ref().map(|c| c.qid.clone()),
        status,
        note: note.trim().to_string(),
        dependency_blocks: if taxon_pending {
            taxon_dependency_statements(row)
        } else {
            Vec::new()
        },
        quickstatements: statements,
    }
}

/// The statements that would create a compound, for a row the run found missing.
///
/// The two SMILES differ in intent: canonical drops stereochemistry, isomeric
/// keeps it, and Wikidata holds both because a structure without stereochemistry
/// is not a structure of the compound but of an unassigned mixture of it.
#[must_use]
pub fn creation_statements(
    row: &CurationInputRow,
    canonical_smiles: &str,
    isomeric_smiles: &str,
) -> String {
    create_compound_statements(row.name.trim(), canonical_smiles, isomeric_smiles)
}

/// SPARQL JSON results, as `QLever` and the WDQS both return them.
///
/// A SPARQL JSON result, as `QLever` and the WDQS both return it: `results` is
/// an object holding a `bindings` array.
///
/// Bytes rather than constructed values, so a change in the wire shape has to be
/// made here deliberately, where the mistake would otherwise be invisible.
#[cfg(test)]
#[cfg(test)]
#[path = "knowledge/tests.rs"]
mod tests;
