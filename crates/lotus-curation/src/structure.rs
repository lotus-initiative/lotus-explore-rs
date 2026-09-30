// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Turning a structure into the key Wikidata matches on.
//!
//! Wikidata identifies a compound by its `InChIKey` (`P235`), and a curator's
//! file has `SMILES`. Something has to convert between them, and that something
//! is a chemistry toolkit. The browser has `RDKit` compiled to WebAssembly; a
//! command line does not, and adding a native toolkit to this workspace to do it
//! would be a build dependency on an LLVM toolchain for one function.
//!
//! So the conversion goes to the same public API the web client already falls
//! back to. The cost is a network round trip per structure and a dependency on
//! somebody else's uptime; the benefit is that the CLI and the web client derive
//! the same key from the same input, and two keys for one molecule is exactly
//! the problem this whole crate exists to prevent.

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
/// The endpoint is a batch endpoint and takes every structure at once, which is
/// the difference between three requests for a file and three hundred for one.
/// The per-row version was written first and was rate-limited on the third row
/// of a five-row file, so this is not an optimisation: the other shape does not
/// work in practice.
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
mod tests {
    #![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

    use super::*;
    use lotus_search::testing::Scripted;

    /// Build a transport from replies made at runtime.
    ///
    /// They leak: a test process is short-lived, and a fixture that must be
    /// spelled as one long literal is a fixture nobody can read.
    fn script(rows: Vec<(u16, String)>) -> Scripted {
        Scripted::new(
            rows.into_iter()
                .map(|(status, body)| (status, &*Box::leak(body.into_boxed_str())))
                .collect(),
        )
    }

    /// A successful conversion row, as the service writes it.
    fn ok(output: &str) -> String {
        format!(r#"{{"success":true,"output":"{output}","error":""}}"#)
    }

    /// A whole batch reply, one successful row per output.
    fn batch(outputs: &[&str]) -> String {
        let rows: Vec<String> = outputs.iter().map(|output| ok(output)).collect();
        format!(r#"{{"results":[{}]}}"#, rows.join(","))
    }

    /// The three replies a full conversion takes, for one structure.
    fn one_structure() -> Vec<(u16, String)> {
        vec![
            (200, batch(&["LFQSCWFLJHTTHZ-UHFFFAOYSA-N"])),
            (200, batch(&["CCO"])),
            (200, batch(&["InChI=1S/C2H6O/c1-2-3/h3H,2H2,1H3"])),
        ]
    }

    /// A batch in which the first structure is unreadable and the second is not.
    fn batch_with_one_bad() -> Vec<(u16, String)> {
        vec![
            (
                200,
                r#"{"results":[{"success":false,"output":"","error":"unparsable"},{"success":true,"output":"KEY2","error":""}]}"#
                    .to_string(),
            ),
            (200, batch(&["CCO", "CCC"])),
        ]
    }

    #[tokio::test]
    async fn a_structure_becomes_the_key_wikidata_is_matched_on() {
        let http = script(one_structure());
        let structure = convert_structure(&http, "CCO")
            .await
            .expect("the conversion succeeds");

        assert_eq!(
            structure.inchikey.as_deref(),
            Some("LFQSCWFLJHTTHZ-UHFFFAOYSA-N")
        );
        assert!(structure.is_known());
        assert_eq!(
            structure.structure_key().inchikey.as_deref(),
            Some("LFQSCWFLJHTTHZ-UHFFFAOYSA-N")
        );
    }

    #[tokio::test]
    async fn a_whole_file_costs_three_requests_and_not_three_per_row() {
        // Why this is batched: the per-row version was rate-limited on the third
        // row of a five-row file. This is the assertion that keeps it that way.
        let http = script(vec![
            (200, batch(&["A", "B", "C", "D", "E"])),
            (200, batch(&["CCO", "CCC", "CCN", "CCC", "CCO"])),
            (200, batch(&["i1", "i2", "i3", "i4", "i5"])),
        ]);

        let rows = ["CCO", "CCC", "CCN", "CCC", "CCO"];
        let converted = convert_structures(&http, &rows)
            .await
            .expect("the file converts");

        assert_eq!(converted.len(), 5);
        assert_eq!(http.call_count(), 3, "three requests for the whole file");
    }

    #[tokio::test]
    async fn every_structure_is_sent_in_one_request() {
        let http = script(vec![
            (200, batch(&["A", "B"])),
            (200, batch(&["CCO", "CCC"])),
            (200, batch(&["x", "y"])),
        ]);

        let _ = convert_structures(&http, &["CCO", "CCC"]).await;

        let asked = http.queries();
        assert!(asked[0].contains(r#""CCO""#), "{asked:?}");
        assert!(
            asked[0].contains(r#""CCC""#),
            "both structures in one request: {asked:?}"
        );
    }

    #[tokio::test]
    async fn the_key_is_asked_for_first_and_there_is_no_isomeric_output() {
        let http = script(one_structure());
        let _ = convert_structure(&http, "CCO").await;

        let asked = http.queries();
        assert!(asked[0].contains("output_format=inchikey"), "{asked:?}");
        assert!(
            asked[1].contains("output_format=canonicalsmiles"),
            "{asked:?}"
        );
        // The service has no isomeric-SMILES output; asking would fail.
        assert!(
            !asked.iter().any(|call| call.contains("isomeric")),
            "the service has no isomeric output: {asked:?}"
        );
    }

    #[tokio::test]
    async fn the_structures_are_sent_as_smiles_and_trimmed() {
        let http = script(one_structure());
        let _ = convert_structure(&http, " CCO ").await;

        // A spreadsheet cell with a stray space still converts.
        assert!(
            http.queries()[0].contains(r#""CCO""#),
            "{:?}",
            http.queries()
        );
    }

    #[tokio::test]
    async fn a_structure_the_service_cannot_read_is_invalid_input() {
        let http = script(batch_with_one_bad());

        let err = convert_structures(&http, &["bad", "CCO"])
            .await
            .expect_err("the unreadable structure is reported");

        assert!(
            matches!(err, CurationError::InvalidInput(ref m) if m.contains("unparsable")),
            "{err:?}"
        );
        // A fault in the row, so retrying changes nothing.
        assert!(!err.is_recoverable(), "{err:?}");
    }

    #[tokio::test]
    async fn a_reply_that_is_short_is_a_parse_error_not_a_missing_row() {
        // If the service answers for fewer structures than it was sent, the
        // missing ones must not read as "this structure is absent" -- that is how
        // a compound nobody looked up gets reported as new.
        let http = script(vec![(200, batch(&["A"]))]);

        let err = convert_structures(&http, &["CCO", "CCC"])
            .await
            .expect_err("a short reply is not an answer for the file");

        assert!(matches!(err, CurationError::Parse(_)), "{err:?}");
        assert!(err.to_string().contains("1 of 2"), "{err}");
    }

    #[tokio::test]
    async fn an_empty_structure_is_refused_before_any_request() {
        let http = script(vec![]);
        let err = convert_structure(&http, "   ")
            .await
            .expect_err("there is nothing to convert");
        assert!(matches!(err, CurationError::InvalidInput(_)), "{err:?}");
        assert_eq!(
            http.call_count(),
            0,
            "no request for a row with no structure"
        );
    }

    #[tokio::test]
    async fn a_key_that_comes_back_empty_is_not_a_key() {
        // An empty output is how the service says "I have no answer", and
        // treating it as a key would query Wikidata for the empty string.
        let http = script(vec![(200, batch(&[""]))]);

        let err = convert_structure(&http, "CCO")
            .await
            .expect_err("an empty output is not a key");

        assert!(matches!(err, CurationError::Parse(_)), "{err:?}");
    }

    #[tokio::test]
    async fn a_rejected_conversion_is_a_transport_error_naming_the_rate_limit() {
        let http = script(vec![(429, "slow down".into())]);
        let err = convert_structure(&http, "CCO")
            .await
            .expect_err("a 429 is not a conversion");
        assert!(matches!(err, CurationError::Http(_)), "{err:?}");
        assert!(
            err.is_recoverable(),
            "a 429 is what a retry is for: {err:?}"
        );
        // The one status a user can act on by asking for less.
        assert!(err.to_string().contains("rate-limiting"), "{err}");
    }

    #[tokio::test]
    async fn a_missing_inchi_does_not_fail_the_conversion() {
        // The `InChI` is a bonus; the key is what the run turns on.
        let http = script(vec![
            (200, batch(&["LFQSCWFLJHTTHZ-UHFFFAOYSA-N"])),
            (200, batch(&["CCO"])),
            (500, "no".into()),
        ]);

        let structure = convert_structure(&http, "CCO")
            .await
            .expect("the run continues");

        assert!(structure.is_known());
        assert_eq!(
            structure.inchi, None,
            "the InChI is optional, the key is not"
        );
    }

    #[tokio::test]
    async fn no_structures_is_refused_rather_than_answered_with_nothing() {
        let http = script(vec![]);
        let err = convert_structures(&http, &[])
            .await
            .expect_err("an empty file is not a conversion");
        assert!(matches!(err, CurationError::InvalidInput(_)), "{err:?}");
        assert_eq!(http.call_count(), 0);
    }

    #[test]
    fn a_success_with_no_output_is_a_parse_failure() {
        // The service answered, and what it answered was not a structure. That is
        // a shape this does not read, so it is reported as a parse failure rather
        // than quietly curating an empty value.
        for blank in ["", "   ", "\n\t "] {
            let converted = Converted {
                output: Some(blank.to_owned()),
                error: None,
            };
            let result = converted.into_result("SMILES");
            assert!(
                matches!(result, Err(CurationError::Parse(ref message)) if message.contains("no SMILES")),
                "{blank:?} is not an output, got {result:?}"
            );
        }
    }

    #[test]
    fn a_real_output_is_trimmed_and_kept() {
        let result = Converted {
            output: Some("  CCO  ".to_owned()),
            error: None,
        }
        .into_result("SMILES");
        assert_eq!(result.unwrap_or_default().as_deref(), Some("CCO"));
    }
}
