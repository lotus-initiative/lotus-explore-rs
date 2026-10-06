// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
// The search future is not `Send`: `on_phase` is a closure over a Dioxus
// `Signal`, which is a `RefCell`. See the full explanation in
// `features/explore/executor.rs`, which is the same cause reached from here.
#![expect(
    clippy::future_not_send,
    reason = "`on_phase` captures a Dioxus `Signal`, which is a `RefCell` and not `Sync`"
)]
//! Taxon resolution service — maps a free-text name to a Wikidata QID.

mod match_selection;

use crate::features::explore::search_metrics::SearchMetrics;
use crate::features::explore::types::{
    DomainError, LookupNotice, ParseFault, QueryStage, ValidationFault,
};
use crate::features::explore::{search_utils::sanitize_taxon_input, taxon_cache};
use crate::perf;
use crate::repositories::LotusRepository;
use crate::services::search_telemetry as telemetry;
use crate::sparql;
use lotus_model::TaxonMatch;

/// Output of a successful taxon resolution.
#[derive(Debug, Clone)]
pub struct TaxonResolution {
    /// The resolved Wikidata QID (e.g. `"Q12345"`), or `None` if the criteria
    /// contained no taxon, or `Some("*")` for the "all taxa" wildcard.
    pub qid: Option<String>,
    /// Everything worth telling the user about how this name resolved.
    ///
    /// A list rather than an `Option` because a single lookup can raise more than
    /// one: `bacteria` is both spelled differently from `Bacteria` and ambiguous.
    /// Collapsing them to one would mean deciding which to drop, and that decision
    /// is not the resolver's to make.
    pub warnings: Vec<LookupNotice>,
}

#[must_use]
// Bounds are established two lines below: every index reads `bytes[0]` and
// `bytes[1]` only after the `bytes.len() > 1` guard, and reading the first
// byte of a non-empty slice can never panic.
#[allow(
    clippy::indexing_slicing,
    reason = "both reads are behind a `len() > 1` guard, and byte 0 of a non-empty slice is always in bounds"
)]
pub fn requires_remote_lookup(taxon: &str) -> bool {
    if taxon.is_empty() || taxon == "*" {
        return false;
    }
    // QIDs are always ASCII — use byte-level check to avoid Unicode iterator.
    let bytes = taxon.as_bytes();
    !(bytes.len() > 1
        && matches!(bytes[0], b'Q' | b'q')
        && bytes[1..].iter().all(u8::is_ascii_digit))
}

/// Resolve a free-text taxon name (or QID, or wildcard) to a Wikidata QID.
/// Returns:
pub async fn resolve<R: LotusRepository>(
    taxon: &str,
    repo: &R,
    metrics: &mut SearchMetrics,
) -> Result<TaxonResolution, DomainError> {
    if let Some(resolution) = immediate_resolution(taxon) {
        return Ok(resolution);
    }
    // Pass a bare Wikidata QID directly — no SPARQL round-trip needed.
    // Accepts both 'Q' and 'q' prefix; the slice `&taxon[1..]` is safe since
    // 'Q'/'q' are single-byte ASCII characters.
    // At this point taxon is neither empty nor "*" (both handled above).
    if !requires_remote_lookup(taxon) {
        return Ok(TaxonResolution {
            qid: Some(taxon.to_uppercase()),
            warnings: Vec::new(),
        });
    }

    // A guard rather than a handle: this function returns from three places below,
    // and only the first closed the timer. The other two -- including the SPARQL
    // path most searches take -- left `LOTUS:taxon_resolution` open for the rest
    // of the session, so every later resolution found the label taken and
    // reported a duration measured from here.
    let taxon_timer = perf::Timer::start("LOTUS:taxon_resolution");
    let sanitized = sanitize_taxon_input(taxon);

    let standardized_warning = (sanitized != taxon).then(|| LookupNotice::Standardized {
        original: taxon.into(),
        standardized: sanitized.clone(),
    });

    // Fast path: the notice comes back out of the cache rather than being
    // recomputed, so a repeat search reports what the first one reported.
    if let Some(cached) = taxon_cache::lookup(&sanitized) {
        let taxon_elapsed = taxon_timer.end();
        telemetry::taxon_cache_hit(taxon_elapsed, &sanitized, &cached.qid);
        let warnings = notices(standardized_warning, &cached);
        return Ok(TaxonResolution {
            qid: Some(cached.qid),
            warnings,
        });
    }

    // Slow path: SPARQL, scientific name first.
    //
    // The common-name lookup is a *second* round trip rather than a second branch
    // of this one, and that is the whole design. The scientific lookup is index
    // served and answers in about 0.3s; the common-name lookup cannot be (every
    // `P1843` value is language-tagged and inconsistently capitalised, so comparing
    // lexical forms is a scan of every statement -- about 2.5s measured). A UNION
    // would make every taxon search pay that 2.5s, including the overwhelming
    // majority that hit a scientific name and never needed it.
    let mut matches = lookup(repo, metrics, lotus_query::taxon_lookup_query(&sanitized)).await?;

    if matches.is_empty() {
        // No scientific name, so try the common one. Resolving rather than refusing is
        // deliberate: sending the reader to Wikidata to redo this lookup is a
        // worse answer than a labelled one.
        matches = lookup(
            repo,
            metrics,
            lotus_query::taxon_common_name_lookup_query(&sanitized),
        )
        .await?;
    }

    if matches.is_empty() {
        return Err(DomainError::Validation(ValidationFault::TaxonNotFound {
            input: taxon.into(),
        }));
    }

    let selection = match_selection::pick_best_match(&sanitized, &matches)?;
    let cached = selection.to_cached();
    let warnings = notices(standardized_warning, &cached);

    taxon_cache::store(&sanitized, &cached);
    Ok(TaxonResolution {
        qid: Some(cached.qid),
        warnings,
    })
}

/// Run one taxon lookup and parse it, charging the round trip to `metrics`.
///
/// A transport failure here is fatal rather than falling through to the next
/// lookup: if the endpoint is unreachable, the common-name scan would be too,
/// and two failed round trips is a slower way to learn the same thing.
async fn lookup<R: LotusRepository>(
    repo: &R,
    metrics: &mut SearchMetrics,
    query: String,
) -> Result<Vec<TaxonMatch>, DomainError> {
    // Its own label, not `LOTUS:taxon_resolution`: a `console.time` label holds one
    // timer document-wide, so a nested site sharing the outer label is refused by
    // the browser and its `timeEnd` closes the outer one instead.
    let timer = perf::start_timer("LOTUS:taxon_lookup");
    let csv = repo.sparql_body(&query).await.map_err(|error| {
        let _ = perf::end_timer("LOTUS:taxon_lookup", timer);
        DomainError::Transport {
            stage: QueryStage::TaxonSearch,
            source: error,
        }
    })?;

    let elapsed = perf::end_timer("LOTUS:taxon_lookup", timer);
    metrics.add_network(elapsed);
    telemetry::taxon_sparql_done(elapsed);

    sparql::parse_taxon_csv(csv.as_ref()).map_err(|e| {
        DomainError::Parse(ParseFault::TaxonCsv {
            details: e.to_string(),
        })
    })
}

/// Everything true about how `taxon` resolved, in reading order.
///
/// Standardized first because it is about what was typed; ambiguous second
/// because it is about what the typing turned out to mean. Built here rather
/// than at either call site, so the query path and the cache path cannot order
/// them differently -- which is the bug this replaced.
fn notices(
    standardized: Option<LookupNotice>,
    cached: &taxon_cache::CachedTaxon,
) -> Vec<LookupNotice> {
    let mut warnings: Vec<LookupNotice> = standardized.into_iter().collect();
    warnings.extend(cached.warnings());
    warnings
}

fn immediate_resolution(taxon: &str) -> Option<TaxonResolution> {
    match taxon {
        "" => Some(TaxonResolution {
            qid: None,
            warnings: Vec::new(),
        }),
        "*" => Some(TaxonResolution {
            qid: Some("*".into()),
            warnings: Vec::new(),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
