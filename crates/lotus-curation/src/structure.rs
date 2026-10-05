// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Turning a structure into the key Wikidata matches on.
//!
//! Wikidata identifies a compound by its `InChIKey` (`P235`); a curator's file has
//! `SMILES`. The browser has `RDKit` compiled to WebAssembly, a command line does
//! not, and a native toolkit here would mean an LLVM toolchain build dependency
//! for one function.
//!
//! So the conversion goes to the same public API the web client already falls
//! back to: a network round trip per structure and a dependency on somebody
//! else's uptime, in exchange for CLI and web client deriving the same key from
//! the same input. Two keys for one molecule is the problem this crate exists to
//! prevent.

use lotus_search::{Http, HttpResponse as _};

use crate::{CurationError, NATPROD_API_BASE};

/// A structure, in the forms Wikidata and a `QuickStatements` bundle want.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConvertedStructure {
    /// The `InChIKey`, which is how the compound is looked up.
    pub inchikey: Option<String>,
    /// Canonical `SMILES`, which drops stereochemistry.
    pub canonical_smiles: Option<String>,
    /// Isomeric `SMILES`, which keeps it.
    pub isomeric_smiles: Option<String>,
    /// The `InChI`.
    pub inchi: Option<String>,
}

impl ConvertedStructure {
    /// The lookup key, if the conversion produced one.
    #[must_use]
    pub fn structure_key(&self) -> crate::StructureKey {
        crate::StructureKey {
            inchikey: self.inchikey.clone(),
        }
    }

    /// Whether the conversion produced anything usable.
    ///
    /// A structure with no `InChIKey` is not an error: the input may have been a
    /// molfile, or the service may not know the compound. It does mean the row
    /// cannot be looked up, and curation says so rather than guessing.
    #[must_use]
    pub fn is_known(&self) -> bool {
        self.inchikey
            .as_deref()
            .is_some_and(|key| !key.trim().is_empty())
    }
}

/// The URL the batch converter is called at, for one output format.
fn batch_url(output_format: &str) -> String {
    format!("{NATPROD_API_BASE}/convert/batch?output_format={output_format}")
}

/// What the service says about one structure.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Converted {
    /// The output, when the conversion worked.
    output: Option<String>,
    /// Why it did not, when it did not.
    error: Option<String>,
}

impl Converted {
    /// The output, or the reason there is not one.
    fn into_result(self, what: &str) -> Result<Option<String>, CurationError> {
        match (self.output, self.error) {
            (Some(output), _) if !output.trim().is_empty() => Ok(Some(output.trim().to_owned())),
            // A row that succeeded with no output is a shape this does not read,
            // which is a parse failure rather than a failed conversion.
            (Some(_), _) | (None, None) => Err(CurationError::Parse(format!(
                "the structure conversion returned no {what}"
            ))),
            // The service answered `success: false`: the request was fine and the
            // answer is that this structure is not readable, which is a fault in
            // the input and not in the transport.
            (None, Some(error)) => Err(CurationError::InvalidInput(format!(
                "the structure could not be read: {error}"
            ))),
        }
    }
}

/// Ask for one representation of every structure, in a single request.
///
/// The endpoint takes every structure at once: three requests for a file instead
/// of three hundred. The per-row version was written first and was rate-limited on
/// the third row of a five-row file, so this is not an optimisation — the other
/// shape does not work.
///
/// # Errors
/// Returns [`CurationError::Http`] if the service cannot be reached or refuses
/// the batch, and [`CurationError::Parse`] if the reply is not a shape this
/// reads. A structure the service could not convert is *not* an error here: it
/// comes back as a per-row failure, because one unreadable structure in a file
/// is a fact about that row and not about the file.
async fn convert_all<H: Http>(
    http: &H,
    smiles: &[&str],
    output_format: &str,
) -> Result<Vec<Converted>, CurationError> {
    let inputs: Vec<serde_json::Value> = smiles
        .iter()
        .map(|smiles| serde_json::json!({ "value": smiles.trim(), "input_format": "smiles" }))
        .collect();
    let payload = serde_json::json!({ "inputs": inputs }).to_string();

    let response = http
        .post_json(&batch_url(output_format), payload)
        .await
        .map_err(|e| {
            CurationError::Http(format!("the structure conversion request failed: {e}"))
        })?;

    if response.status() >= 400 {
        // 429 is the one worth naming: the service is asking for less traffic,
        // and the answer is to send less of it or to wait.
        let hint = if response.status() == 429 {
            " (the service is rate-limiting; wait a moment and run it again)"
        } else {
            ""
        };
        return Err(CurationError::Http(format!(
            "the structure conversion was refused with HTTP {}{hint}",
            response.status()
        )));
    }

    let text = response
        .text()
        .await
        .map_err(|e| CurationError::Parse(format!("the structure conversion reply: {e}")))?;

    let parsed: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| CurationError::Parse(e.to_string()))?;

    let results = parsed
        .get("results")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            CurationError::Parse("the structure conversion returned no result rows".into())
        })?;

    // A short reply means some structures were silently dropped, and a missing
    // row must not read as a failed conversion: that is how a compound nobody
    // looked up gets reported as new.
    if results.len() < smiles.len() {
        return Err(CurationError::Parse(format!(
            "the structure conversion answered for {} of {} structures",
            results.len(),
            smiles.len()
        )));
    }

    Ok(results
        .iter()
        .map(|row| {
            let succeeded = row
                .get("success")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            Converted {
                output: if succeeded {
                    row.get("output")
                        .and_then(serde_json::Value::as_str)
                        .map(std::borrow::ToOwned::to_owned)
                } else {
                    None
                },
                error: row
                    .get("error")
                    .and_then(serde_json::Value::as_str)
                    .filter(|error| !error.is_empty())
                    .map(std::borrow::ToOwned::to_owned),
            }
        })
        .collect())
}

/// Convert a whole file of structures into the forms curation needs.
///
/// Three requests for the whole file, whatever its size. The `InChIKey` is the
/// one that cannot be derived any other way, so if it fails there is no point
/// asking for the other two.
///
/// # Errors
/// Returns [`CurationError::InvalidInput`] when there are no structures at all,
/// and [`CurationError::Http`] when the service cannot be reached.
pub async fn convert_structures<H: Http>(
    http: &H,
    rows: &[&str],
) -> Result<Vec<ConvertedStructure>, CurationError> {
    if rows.is_empty() {
        return Err(CurationError::InvalidInput(
            "there are no structures to convert".into(),
        ));
    }

    let keys = convert_all(http, rows, "inchikey").await?;
    let canonical = convert_all(http, rows, "canonicalsmiles").await?;
    // The `InChI` is a bonus, and a service that will not produce one should not
    // cost the caller the two it did answer.
    let inchi = convert_all(http, rows, "inchi").await.unwrap_or_default();

    rows.iter()
        .enumerate()
        .map(|(index, _)| {
            Ok(ConvertedStructure {
                // A structure the service could not read has no key, and that is
                // what makes the row an error rather than a new compound.
                inchikey: keys
                    .get(index)
                    .cloned()
                    .map(|converted| converted.into_result("output"))
                    .transpose()?
                    .flatten(),
                canonical_smiles: canonical
                    .get(index)
                    .cloned()
                    .map(|converted| converted.into_result("output"))
                    .transpose()?
                    .flatten(),
                // The service has no isomeric-SMILES output -- it answers
                // "Unsupported output format" -- so there is nothing to ask for.
                // Its canonical form already carries stereochemistry
                // (`C[C@@H](C(=O)O)N` for L-alanine), so a second copy of it here
                // would look like a fact about the compound rather than a copy.
                isomeric_smiles: None,
                inchi: inchi
                    .get(index)
                    .cloned()
                    .map(|converted| converted.into_result("output"))
                    .transpose()?
                    .flatten(),
            })
        })
        .collect()
}

/// Convert a single structure, for a caller that has only one.
///
/// # Errors
/// As [`convert_structures`].
pub async fn convert_structure<H: Http>(
    http: &H,
    smiles: &str,
) -> Result<ConvertedStructure, CurationError> {
    if smiles.trim().is_empty() {
        return Err(CurationError::InvalidInput(
            "the row has no structure".into(),
        ));
    }
    convert_structures(http, &[smiles])
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| CurationError::Parse("the structure conversion returned nothing".into()))
}

#[cfg(test)]
#[path = "structure/tests.rs"]
mod tests;
