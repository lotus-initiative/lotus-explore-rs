// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Reference DOI or QID resolution.

use lotus_model::is_qid;
use lotus_query::reference_by_doi_query;

use crate::client::Http;
use crate::error::{FetchError, ResponseFormat};

/// Resolve a reference field, which is a DOI or a QID, to the QID to filter on.
///
/// # Errors
/// Returns [`FetchError`] if the lookup cannot be run or read, and
/// [`FetchError::Parse`] if the DOI matched no reference.
pub async fn resolve_reference<H: Http>(
    http: &H,
    input: &str,
) -> Result<Option<String>, FetchError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if is_qid(trimmed) {
        return Ok(Some(trimmed.to_ascii_uppercase()));
    }

    let csv =
        crate::execute_with_fallback(http, &reference_by_doi_query(trimmed), ResponseFormat::Csv)
            .await?
            .text()?;
    let matches = lotus_query::parse_reference_lookup_csv(csv.as_bytes())
        .map_err(|e| FetchError::Parse(e.to_string()))?;

    matches
        .first()
        .map(|found| found.qid.clone())
        .map(Some)
        .ok_or_else(|| FetchError::Parse(format!("no reference matched {trimmed:?}")))
}
