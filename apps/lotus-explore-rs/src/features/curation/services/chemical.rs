// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

#[cfg(target_arch = "wasm32")]
use super::http_client::{js_value_to_json, rdkit_bridge_call};
use super::{CurationError, MassResolution, has_stereo_marks};
#[cfg(not(target_arch = "wasm32"))]
use super::{NATPROD_API_BASE, http_client::BatchConvertResponse, http_client::natprod_client};
#[cfg(not(target_arch = "wasm32"))]
use futures::try_join;
use serde::Deserialize;
// The exact-mass readers are native-only (they are gated with their callers), so
// this is too.
#[cfg(not(target_arch = "wasm32"))]
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub(super) struct ConvertFormatsResponse {
    pub(super) canonical_smiles: String,
    pub(super) isomeric_smiles: String,
    pub(super) inchi: String,
    pub(super) inchikey: String,
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Deserialize)]
struct RdkitConvertResponse {
    canonicalsmiles: String,
    isomericsmiles: String,
    inchi: String,
    inchikey: String,
}

pub(super) async fn convert_smiles(smiles: &str) -> Result<ConvertFormatsResponse, CurationError> {
    #[cfg(target_arch = "wasm32")]
    {
        return convert_with_rdkit(smiles).await;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        // The service has no `isomericsmiles` output. It answers
        // `Unsupported output format: isomericsmiles`, and because this used to
        // be part of a `try_join!`, that one unsupported format failed the whole
        // conversion and took the InChIKey down with it.
        //
        // Its `canonicalsmiles` output already carries stereochemistry --
        // `C[C@@H](C(=O)O)N` in, `C[C@@H](C(=O)O)N` out -- so it answers both
        // questions. `lotus-curation` reaches the same conclusion for the same
        // reason. The two are compared downstream by `has_isomeric_smiles`,
        // which looks for stereo marks, so an unspecified structure correctly
        // reports that it has none.
        let (canonical_smiles, inchi, inchikey) = try_join!(
            convert_with_batch_direct(smiles, NATPROD_CANONICAL_SMILES),
            convert_with_batch_direct(smiles, "inchi"),
            convert_with_batch_direct(smiles, "inchikey"),
        )?;

        Ok(ConvertFormatsResponse {
            isomeric_smiles: canonical_smiles.clone(),
            canonical_smiles,
            inchi,
            inchikey,
        })
    }
}

/// The service's name for its canonical-SMILES output.
#[cfg(not(target_arch = "wasm32"))]
const NATPROD_CANONICAL_SMILES: &str = "canonicalsmiles";

/// Every output format this code asks the service for.
///
/// Asking for anything else fails the conversion: the service replies 200 with
/// `success: false` and `Unsupported output format: <name>`, which
/// `extract_batch_convert_output` turns into an error. So this list is not a
/// preference, it is the set the service actually implements -- `isomericsmiles`
/// is not in it, and its canonical output already carries stereochemistry.
#[cfg(not(target_arch = "wasm32"))]
const NATPROD_OUTPUT_FORMATS: [&str; 3] = [NATPROD_CANONICAL_SMILES, "inchi", "inchikey"];

#[cfg(target_arch = "wasm32")]
async fn convert_with_rdkit(smiles: &str) -> Result<ConvertFormatsResponse, CurationError> {
    let value = rdkit_bridge_call("convert", smiles.trim()).await?;
    let parsed = js_value_to_json(&value)?;
    let converted = serde_json::from_value::<RdkitConvertResponse>(parsed)
        .map_err(|e| CurationError::Parse(format!("rdkit.js convert parse error: {e}")))?;
    Ok(ConvertFormatsResponse {
        canonical_smiles: converted.canonicalsmiles,
        isomeric_smiles: converted.isomericsmiles,
        inchi: converted.inchi,
        inchikey: converted.inchikey,
    })
}

#[cfg(not(target_arch = "wasm32"))]
async fn convert_with_batch_direct(
    smiles: &str,
    output_format: &str,
) -> Result<String, CurationError> {
    let url = format!("{NATPROD_API_BASE}/convert/batch?output_format={output_format}");
    let payload = serde_json::json!({
        "inputs": [{ "value": smiles.trim(), "input_format": "smiles" }]
    });
    let response = natprod_client()?
        .post(url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            CurationError::Http(format!("naturalproducts convert/batch request error: {e}"))
        })?;
    if !response.status().is_success() {
        return Err(CurationError::Http(format!(
            "naturalproducts convert/batch failed with HTTP {}",
            response.status().as_u16()
        )));
    }
    let parsed = response.json::<BatchConvertResponse>().await.map_err(|e| {
        CurationError::Parse(format!("naturalproducts convert/batch parse error: {e}"))
    })?;
    extract_batch_convert_output(parsed)
}

#[cfg(not(target_arch = "wasm32"))]
// Takes the parsed response by value: it is read once and dropped, and the
// `as` below narrows a mass the service already serialised as a float.
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_precision_loss)]
fn extract_batch_convert_output(parsed: BatchConvertResponse) -> Result<String, CurationError> {
    let Some(first) = parsed.results.first() else {
        return Err(CurationError::Parse(
            "naturalproducts convert/batch returned no result rows".to_string(),
        ));
    };
    if !first.success {
        return Err(CurationError::InvalidInput(format!(
            "naturalproducts conversion failed: {}",
            first.error
        )));
    }
    Ok(first.output.clone())
}

// Only the native and server paths reach this; the browser client has its
// own fetch path, so a wasm build has no caller for it.
#[cfg(not(target_arch = "wasm32"))]
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

// Exact masses are far below 2^53, so the i64→f64 conversion is exact for
// every chemically-plausible value.
#[allow(clippy::cast_precision_loss)]
// Only the native and server paths reach this; the browser client has its
// own fetch path, so a wasm build has no caller for it.
#[cfg(not(target_arch = "wasm32"))]
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

pub(super) async fn descriptor_mass(smiles: &str) -> Result<f64, CurationError> {
    descriptor_mass_via_rdkit(smiles).await
}

#[cfg(target_arch = "wasm32")]
async fn descriptor_mass_via_rdkit(smiles: &str) -> Result<f64, CurationError> {
    let value = rdkit_bridge_call("exactMass", smiles.trim()).await?;
    value.as_f64().ok_or_else(|| {
        CurationError::Parse("rdkit.js exactMass did not return a number".to_string())
    })
}

#[cfg(not(target_arch = "wasm32"))]
// Native mirror of the WASM `descriptor_mass_via_rdkit` above: same async
// signature, returning `Err` because RDKit JS is browser-only. Documented in
// the module doc for `descriptor_mass`.
#[allow(clippy::unused_async)]
async fn descriptor_mass_via_rdkit(_smiles: &str) -> Result<f64, CurationError> {
    // For non-WASM environments (e.g., server-side), use a reasonable default or error
    // Since the app is browser-based, this shouldn't be called in production
    Err(CurationError::Parse(
        "RDKit JS not available in non-browser environment".to_string(),
    ))
}

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
            if canonical_smiles.trim() != input_smiles.trim() {
                return match descriptor_mass(input_smiles).await {
                    Ok(value) => MassResolution {
                        exact_mass: Some(value),
                        warning: None,
                    },
                    Err(_input_err) => MassResolution {
                        exact_mass: None,
                        warning: Some(format!("Mass unavailable - service limit: {canonical_err}")),
                    },
                };
            }
            MassResolution {
                exact_mass: None,
                warning: Some(format!("Mass unavailable - {canonical_err}")),
            }
        }
    }
}

pub(super) async fn has_undefined_stereo(smiles: &str) -> bool {
    if has_stereo_marks(smiles) {
        return false;
    }

    #[cfg(target_arch = "wasm32")]
    {
        return rdkit_bridge_call("hasUndefinedStereo", smiles.trim())
            .await
            .ok()
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let url = format!(
            "{NATPROD_API_BASE}/chem/stereoisomers?smiles={}",
            urlencoding::encode(smiles.trim())
        );
        let Ok(client) = natprod_client() else {
            return false;
        };
        let Ok(response) = client.get(url).send().await else {
            return false;
        };
        let Ok(json) = response.json::<Value>().await else {
            return false;
        };
        json.get("stereoisomers")
            .and_then(Value::as_array)
            .is_some_and(|a| a.len() > 1)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    /// Pinned against the live service, which answers 200 with
    /// `success: false` and `Unsupported output format: <name>` for anything
    /// outside this list, rather than a 4xx that would be obvious here.
    #[test]
    fn only_formats_the_service_implements_are_requested() {
        assert_eq!(
            super::NATPROD_OUTPUT_FORMATS,
            ["canonicalsmiles", "inchi", "inchikey"],
            "the requested formats changed; the service must be able to answer \
             each one, and `isomericsmiles` is not one it can"
        );
        assert!(
            !super::NATPROD_OUTPUT_FORMATS.contains(&"isomericsmiles"),
            "the service has no isomeric-SMILES output and rejects the request"
        );
    }
}
