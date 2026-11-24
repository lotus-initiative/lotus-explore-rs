// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Taxon scientific-name and common-name resolution with homonym disambiguation.

use lotus_model::{TaxonMatch, is_qid, standardize_taxon_name};
use lotus_query::{parse_taxon_csv, taxon_common_name_lookup_query, taxon_lookup_query};

use crate::client::Http;
use crate::error::{FetchError, ResponseFormat};
use crate::result::{TaxonNote, TaxonResolution};

/// Resolve a taxon's input to a QID.
///
/// An empty input means "every taxon", a `*` is the explicit form of that, and
/// anything shaped like a QID is used without a round trip.
///
/// # Errors
/// Returns [`FetchError`] if the lookup cannot be run or read, and
/// [`FetchError::Parse`] if nothing matched — which is a validation failure the
/// caller reports to the user, not a broken query.
pub async fn resolve_taxon<H: Http>(http: &H, input: &str) -> Result<TaxonResolution, FetchError> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed == "*" {
        return Ok(TaxonResolution {
            qid: None,
            looked_up: trimmed.to_string(),
            notes: Vec::new(),
        });
    }
    if is_qid(trimmed) {
        return Ok(TaxonResolution {
            qid: Some(trimmed.to_ascii_uppercase()),
            looked_up: trimmed.to_string(),
            notes: Vec::new(),
        });
    }

    // Underscores and a lower-case genus both arrive from spreadsheets, and a
    // lookup for the text as typed would find nothing.
    let looked_up = standardize_taxon_name(trimmed);
    let csv =
        crate::execute_with_fallback(http, &taxon_lookup_query(&looked_up), ResponseFormat::Csv)
            .await?
            .text()?;
    let mut candidates = parse_taxon_csv(csv.as_bytes())?;
    if candidates.is_empty() {
        let fallback_csv = crate::execute_with_fallback(
            http,
            &taxon_common_name_lookup_query(&looked_up),
            ResponseFormat::Csv,
        )
        .await?
        .text()?;
        candidates = parse_taxon_csv(fallback_csv.as_bytes())?;
    }

    let Some(mut chosen) = pick_match(&looked_up, &candidates) else {
        return Err(FetchError::Parse(format!("no taxon matched {trimmed:?}")));
    };
    // Homonyms: use the first that has recorded compounds. A probe that fails
    // decides nothing, and the first match stays chosen.
    let homonyms = || {
        candidates
            .iter()
            .filter(|c| c.name.eq_ignore_ascii_case(&looked_up))
    };
    if homonyms().count() > 1 {
        for candidate in homonyms() {
            let probe = lotus_query::taxon_has_compounds_query(&candidate.qid);
            if let Ok(answer) =
                crate::execute_with_fallback(http, &probe, ResponseFormat::Csv).await
                && answer
                    .text()
                    .is_ok_and(|csv| lotus_query::has_data_row(csv.as_bytes()))
            {
                chosen.clone_from(&candidate.qid);
                break;
            }
        }
    }

    let mut notes = Vec::new();
    if looked_up != trimmed {
        notes.push(TaxonNote::Standardized {
            original: trimmed.to_string(),
            looked_up: looked_up.clone(),
        });
    }
    if is_ambiguous(&candidates) {
        notes.push(TaxonNote::Ambiguous {
            candidates: candidates
                .iter()
                .take(4)
                .map(|c| format!("{} ({})", c.name, c.qid))
                .collect(),
        });
    }

    Ok(TaxonResolution {
        qid: Some(chosen),
        looked_up,
        notes,
    })
}

/// The QID to filter on: an exact name match if there is one, else the first
/// candidate the endpoint returned.
fn pick_match(wanted: &str, candidates: &[TaxonMatch]) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }
    let chosen = candidates
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(wanted))
        .or_else(|| candidates.first())?;
    Some(chosen.qid.clone())
}

/// Whether the answer is ambiguous: several exact names, or several candidates
/// and none of them the name that was asked for.
pub(super) const fn is_ambiguous(candidates: &[TaxonMatch]) -> bool {
    candidates.len() > 1
}
