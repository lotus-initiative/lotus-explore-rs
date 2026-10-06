// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![expect(
    clippy::future_not_send,
    reason = "the resolver runs inside the search future, which holds a Dioxus Signal"
)]
//! Reference resolution — turns a QID or a DOI into the Wikidata item to constrain
//! the search by.
//!
//! The third and smallest of the three resolutions. A reference's only usable
//! field is its title, which is prose — and matching prose against several hundred
//! thousand references has no useful answer, since a great many titles are the same
//! three words.
//!
//! What it does have is two identifiers that each name exactly one item, both
//! recognised by shape before any request is made:
//!
//! | Typed | Lookup |
//! | --- | --- |
//! | `Q23118` | [`reference_by_qid_query`] — a `VALUES`, nothing to match |
//! | `10.1002/andp.18280880206` | [`reference_by_doi_query`] — `P356`, uppercased |
//!
//! Anything else is refused rather than guessed at. There is no third route to
//! try, and a search silently ignoring a mistyped reference is the failure this
//! avoids.
//!
//! [`reference_by_qid_query`]: lotus_query::reference_by_qid_query
//! [`reference_by_doi_query`]: lotus_query::reference_by_doi_query

use crate::features::explore::search_metrics::SearchMetrics;
use crate::features::explore::types::{DomainError, LookupNotice, QueryStage, ValidationFault};
use crate::repositories::LotusRepository;
use lotus_model::{looks_like_doi, looks_like_reference_qid};
use lotus_query::ReferenceMatch;

/// A resolved reference, plus whatever is worth telling the reader about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceResolution {
    /// The Wikidata item, or `None` when the field was left empty.
    ///
    /// `None` means "no constraint", not "not found": a reference matching nothing
    /// is an error, not an absent filter.
    pub qid: Option<String>,
    pub notices: Vec<LookupNotice>,
}

/// Whether this field is worth a round trip at all.
///
/// False for an empty field, so a search not mentioning a reference does not pay
/// for one. Both recognised shapes do pay: a QID is confirmed to exist, and a DOI
/// is a lookup in its own right.
#[must_use]
pub fn requires_remote_lookup(reference: &str) -> bool {
    let trimmed = reference.trim();
    !trimmed.is_empty() && (looks_like_reference_qid(trimmed) || looks_like_doi(trimmed))
}

/// Resolve the reference field.
///
/// # Errors
/// Returns [`DomainError::Validation`] with
/// [`ValidationFault::ReferenceNotAnIdentifier`] for input that is neither a QID
/// nor a DOI, and [`ValidationFault::ReferenceNotFound`] for one that is and that
/// Wikidata does not have. A transport failure is a [`DomainError::Transport`],
/// reported rather than swallowed.
pub async fn resolve<R: LotusRepository>(
    reference: &str,
    repo: &R,
    metrics: &mut SearchMetrics,
) -> Result<ReferenceResolution, DomainError> {
    let trimmed = reference.trim();
    if trimmed.is_empty() {
        return Ok(ReferenceResolution {
            qid: None,
            notices: Vec::new(),
        });
    }

    let query = if looks_like_reference_qid(trimmed) {
        lotus_query::reference_by_qid_query(trimmed)
    } else if looks_like_doi(trimmed) {
        lotus_query::reference_by_doi_query(trimmed)
    } else {
        // Nothing else identifies a reference, so there is nothing to look up.
        // Saying so beats quietly dropping the constraint.
        return Err(DomainError::Validation(
            ValidationFault::ReferenceNotAnIdentifier {
                input: reference.to_owned(),
            },
        ));
    };

    // At most one row can come back -- a QID names one item, and a DOI is unique
    // in Wikidata -- so the first match is the whole answer.
    let found = run(&query, repo, metrics).await?.into_iter().next();
    match found {
        Some(ReferenceMatch { qid }) => Ok(ReferenceResolution {
            qid: Some(qid),
            notices: Vec::new(),
        }),
        None => Err(DomainError::Validation(
            ValidationFault::ReferenceNotFound {
                input: reference.to_owned(),
            },
        )),
    }
}

async fn run<R: LotusRepository>(
    query: &str,
    repo: &R,
    metrics: &mut SearchMetrics,
) -> Result<Vec<ReferenceMatch>, DomainError> {
    let timer = crate::perf::start_timer("LOTUS:reference_resolution");
    let result = repo.sparql_body(query).await;
    let elapsed = crate::perf::end_timer("LOTUS:reference_resolution", timer);
    metrics.add_network(elapsed);
    let csv = result.map_err(|source| DomainError::Transport {
        stage: QueryStage::TaxonSearch,
        source,
    })?;
    let parsed = lotus_query::parse_reference_lookup_csv(&csv).map_err(|error| {
        DomainError::Parse(crate::features::explore::types::ParseFault::ResultsCsv {
            details: error.to_string(),
        })
    })?;

    // An empty result is a legitimate answer and `resolve` turns it into "not
    // found"; reading it as a transport failure would tell the reader to retry
    // instead of correcting the DOI.
    Ok(parsed)
}

#[cfg(test)]
#[path = "resolve_reference/tests.rs"]
mod tests;
