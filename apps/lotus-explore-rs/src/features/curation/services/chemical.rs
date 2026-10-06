// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Structure conversion, mass and stereochemistry, all through the `RDKit`
//! bridge.
//!
//! One path, for every renderer. Branching on the target made a desktop window
//! differ from a browser: the HTTP branch (`api.naturalproducts.net`) answered
//! `Unsupported output format: isomericsmiles`, had no exact-mass endpoint, and
//! made the structure editor and the curation page disagree with the web build.
//!
//! A desktop window is a `WebView`, so `RDKit` runs in it as in a browser. What
//! native cannot do is call `web_sys` -- there is no `window` object to reach --
//! and `document::eval` works in both.

// Every function here awaits `document::eval`, which is a `WebView` round trip
// and so produces a future that is not `Send`. Each is called from a component's
// own single-threaded task, which is the correct shape for it.
// The search future is not `Send`: `on_phase` is a closure over a Dioxus
// `Signal`, which is a `RefCell`. See the full explanation in
// `features/explore/executor.rs`, which is the same cause reached from here.
#![expect(
    clippy::future_not_send,
    reason = "`on_phase` captures a Dioxus `Signal`, which is a `RefCell` and not `Sync`"
)]

use super::{CurationError, MassResolution};
use serde::Deserialize;
#[cfg(test)]
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub(super) struct ConvertFormatsResponse {
    pub(super) canonical_smiles: String,
    pub(super) isomeric_smiles: String,
    pub(super) inchi: String,
    pub(super) inchikey: String,
}

#[derive(Debug, Deserialize)]
struct RdkitConvertResponse {
    canonicalsmiles: String,
    isomericsmiles: String,
    inchi: String,
    inchikey: String,
}

/// Convert a SMILES string to the four identifiers curation needs.
///
/// # Errors
/// Returns a message if the bridge is unavailable, `RDKit` cannot read the
/// structure, or its answer does not parse.
pub(super) async fn convert_smiles(smiles: &str) -> Result<ConvertFormatsResponse, CurationError> {
    let value = super::http_client::rdkit_bridge_call("convert", smiles).await?;
    let parsed = serde_json::from_value::<RdkitConvertResponse>(value)
        .map_err(|e| CurationError::Parse(format!("rdkit.js convert parse error: {e}")))?;
    Ok(ConvertFormatsResponse {
        canonical_smiles: parsed.canonicalsmiles,
        isomeric_smiles: parsed.isomericsmiles,
        inchi: parsed.inchi,
        inchikey: parsed.inchikey,
    })
}

/// The exact molecular mass, in daltons.
///
/// # Errors
/// Returns a message if the bridge is unavailable or `RDKit` does not report a
/// mass for the structure.
pub(super) async fn descriptor_mass(smiles: &str) -> Result<f64, CurationError> {
    let value = super::http_client::rdkit_bridge_call("exactMass", smiles).await?;
    value.as_f64().ok_or_else(|| {
        CurationError::Parse("rdkit.js exactMass did not return a number".to_string())
    })
}

/// Whether `RDKit` reports the structure as having undefined stereocentres.
///
/// # Errors
/// Returns a message if the bridge is unavailable.
pub(super) async fn has_undefined_stereo(smiles: &str) -> Result<bool, CurationError> {
    let value = super::http_client::rdkit_bridge_call("hasUndefinedStereo", smiles).await?;
    Ok(value.as_bool().unwrap_or(false))
}

/// The mass to record, falling back to the input structure.
///
/// A canonical SMILES can be one the mass service will not read -- a mixture, or
/// a structure the toolkit cannot kekulise -- while the input is fine. Trying the
/// input is cheap and turns a missing mass into a warning rather than a gap.
pub(super) async fn resolve_exact_mass(
    input_smiles: &str,
    canonical_smiles: &str,
) -> MassResolution {
    match descriptor_mass(canonical_smiles).await {
        Ok(value) => MassResolution {
            exact_mass: Some(value),
            warning: None,
        },
        Err(canonical_err) => {
            if canonical_smiles.trim() == input_smiles.trim() {
                return MassResolution {
                    exact_mass: None,
                    warning: Some(format!("Mass unavailable - {canonical_err}")),
                };
            }
            descriptor_mass(input_smiles).await.map_or_else(
                |_| MassResolution {
                    exact_mass: None,
                    warning: Some(format!("Mass unavailable - service limit: {canonical_err}")),
                },
                |value| MassResolution {
                    exact_mass: Some(value),
                    warning: None,
                },
            )
        }
    }
}

/// Read an exact mass out of a JSON payload, at any depth.
///
/// Test-only, kept to pin the reading rules: a mass arrives as a number, a
/// numeric string, or with thousands separators.
#[cfg(test)]
pub fn extract_exact_mass_from_json(value: &Value) -> Option<f64> {
    if let Some(v) = value
        .get("exact_molecular_weight")
        .and_then(parse_exact_mass_scalar)
    {
        return Some(v);
    }
    if let Some(obj) = value.as_object() {
        for nested in obj.values() {
            if let Some(v) = extract_exact_mass_from_json(nested) {
                return Some(v);
            }
        }
    }
    if let Some(arr) = value.as_array() {
        for nested in arr {
            if let Some(v) = extract_exact_mass_from_json(nested) {
                return Some(v);
            }
        }
    }
    None
}

/// Exact masses are far below 2^53, so the i64 to f64 conversion is exact for
/// every chemically plausible value.
#[cfg(test)]
#[allow(clippy::cast_precision_loss)]
fn parse_exact_mass_scalar(value: &Value) -> Option<f64> {
    if let Some(v) = value.as_f64() {
        return Some(v);
    }
    if let Some(v) = value.as_i64() {
        return Some(v as f64);
    }
    if let Some(v) = value.as_u64() {
        return Some(v as f64);
    }
    value
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.replace(',', "").parse::<f64>().ok())
}

#[cfg(test)]
#[path = "chemical/tests.rs"]
mod tests;
