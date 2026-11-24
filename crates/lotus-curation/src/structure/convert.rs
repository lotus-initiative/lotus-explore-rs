// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Batch structure conversion client over [`Http`].

use lotus_search::{Http, HttpResponse as _};

use super::ConvertedStructure;
use super::formula::{has_stereo_marks, strip_stereo_marks};
use crate::CurationError;

const NATPROD_API_BASE: &str = "https://api.naturalproducts.net/latest";

/// The URL the batch converter is called at, for one output format.
fn batch_url(output_format: &str) -> String {
    format!("{NATPROD_API_BASE}/convert/batch?output_format={output_format}")
}

/// What the service says about one structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Converted {
    /// The output, when the conversion worked.
    pub(super) output: Option<String>,
    /// Why it did not, when it did not.
    pub(super) error: Option<String>,
}

impl Converted {
    /// The output, or the reason there is not one.
    pub(super) fn into_result(self, what: &str) -> Result<Option<String>, CurationError> {
        match (self.output, self.error) {
            (Some(output), _) if !output.trim().is_empty() => Ok(Some(output.trim().to_owned())),
            (Some(_), _) | (None, None) => Err(CurationError::Parse(format!(
                "the structure conversion returned no {what}"
            ))),
            (None, Some(error)) => Err(CurationError::InvalidInput(format!(
                "the structure could not be read: {error}"
            ))),
        }
    }
}

/// Ask for one representation of every structure, in a single request.
pub(super) async fn convert_all<H: Http>(
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
    convert_rows(http, rows, Unreadable::FailsItsRow).await
}

/// What a structure the service cannot read does to the conversion.
#[derive(Clone, Copy)]
enum Unreadable {
    FailsItsRow,
    IsAnError,
}

async fn convert_rows<H: Http>(
    http: &H,
    rows: &[&str],
    unreadable: Unreadable,
) -> Result<Vec<ConvertedStructure>, CurationError> {
    let keys = convert_all(http, rows, "inchikey").await?;
    let canonical = convert_all(http, rows, "canonicalsmiles").await?;
    let inchi = convert_all(http, rows, "inchi").await.unwrap_or_default();

    (0..rows.len())
        .map(|index| {
            let raw_smiles = row_value(canonical.get(index), unreadable)?;
            let (canonical_smiles, isomeric_smiles) = match raw_smiles {
                Some(smiles) if has_stereo_marks(&smiles) => {
                    (Some(strip_stereo_marks(&smiles)), Some(smiles))
                }
                Some(smiles) => (Some(smiles), None),
                None => (None, None),
            };
            Ok(ConvertedStructure {
                inchikey: row_value(keys.get(index), unreadable)?,
                canonical_smiles,
                isomeric_smiles,
                inchi: row_value(inchi.get(index), unreadable)?,
            })
        })
        .collect()
}

fn row_value(
    converted: Option<&Converted>,
    unreadable: Unreadable,
) -> Result<Option<String>, CurationError> {
    match (
        converted
            .cloned()
            .map(|c| c.into_result("output"))
            .transpose(),
        unreadable,
    ) {
        (Err(CurationError::InvalidInput(_)), Unreadable::FailsItsRow) => Ok(None),
        (other, _) => other.map(Option::flatten),
    }
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
    convert_rows(http, &[smiles], Unreadable::IsAnError)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| CurationError::Parse("the structure conversion returned nothing".into()))
}
