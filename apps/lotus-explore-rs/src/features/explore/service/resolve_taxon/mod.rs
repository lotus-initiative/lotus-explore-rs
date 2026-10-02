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
    /// A list rather than an `Option` because a single lookup can raise more
    /// than one: `bacteria` is both spelled differently from `Bacteria` and
    /// ambiguous. Collapsing them to one would mean deciding which to drop, and
    /// that decision is not the resolver's to make — they are two true things
    /// about the same resolution.
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

    let taxon_timer = perf::start_timer("LOTUS:taxon_resolution");
    let sanitized = sanitize_taxon_input(taxon);

    let standardized_warning = (sanitized != taxon).then(|| LookupNotice::Standardized {
        original: taxon.into(),
        standardized: sanitized.clone(),
    });

    // Fast path: cache hit. The notice comes back out of the cache rather than
    // being recomputed, so a repeat search reports what the first one reported.
    if let Some(cached) = taxon_cache::lookup(&sanitized) {
        let taxon_elapsed = perf::end_timer("LOTUS:taxon_resolution", taxon_timer);
        telemetry::taxon_cache_hit(taxon_elapsed, &sanitized, &cached.qid);
        let warnings = notices(standardized_warning, &cached);
        return Ok(TaxonResolution {
            qid: Some(cached.qid),
            warnings,
        });
    }

    // Slow path: SPARQL query, scientific name first.
    //
    // The common-name lookup is a *second* round trip rather than a second
    // branch of this one, and that is the whole design. The scientific lookup is
    // served from an index and answers in about 0.3s; the common-name lookup
    // cannot be (every `P1843` value is language-tagged and inconsistently
    // capitalised, so it has to compare lexical forms, which is a scan of every
    // statement -- about 2.5s measured). Folding it in with a UNION would make
    // every taxon search pay the 2.5s, including the overwhelming majority that
    // hit a scientific name and never needed it.
    let mut matches = lookup(repo, metrics, lotus_query::taxon_lookup_query(&sanitized)).await?;

    if matches.is_empty() {
        // No scientific name, so try the common one. Resolving rather than
        // refusing is deliberate: sending the reader to Wikidata to perform the
        // lookup this tool just did is a worse answer than a labelled one.
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
    let timer = perf::start_timer("LOTUS:taxon_resolution");
    let csv = repo.sparql_body(&query).await.map_err(|error| {
        let _ = perf::end_timer("LOTUS:taxon_resolution", timer);
        DomainError::Transport {
            stage: QueryStage::TaxonSearch,
            source: error,
        }
    })?;

    let elapsed = perf::end_timer("LOTUS:taxon_resolution", timer);
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
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::unwrap_used)]
    // `LotusRepository` is an async-fn-in-trait, so the stub's two methods must
    // be `async fn`; a canned response has nothing to await, and the lint
    // cannot tell the trait signature apart from a choice in the body.
    #![expect(
        clippy::unused_async_trait_impl,
        reason = "LotusRepository is an AFIT: the `async fn` spelling is fixed by the trait"
    )]

    use super::*;
    use crate::api::SearchResponse;
    use crate::repositories::{LotusRepository, RepositoryError};
    use lotus_search::SearchCriteria;

    /// Stub that always returns a fixed SPARQL CSV response; API not configured.
    #[derive(Clone)]
    struct StubRepo {
        response: Result<lotus_search::ResponseBody, RepositoryError>,
    }

    impl StubRepo {
        fn ok(csv: &str) -> Self {
            Self {
                response: Ok(csv.as_bytes().to_vec().into()),
            }
        }
        fn err_network(msg: &str) -> Self {
            Self {
                response: Err(RepositoryError::network(msg)),
            }
        }
    }

    impl LotusRepository for StubRepo {
        async fn api_search(
            &self,
            _: &SearchCriteria,
            _: usize,
            _: bool,
        ) -> Option<Result<SearchResponse, RepositoryError>> {
            None
        }

        async fn sparql_body(
            &self,
            _: &str,
        ) -> Result<lotus_search::ResponseBody, RepositoryError> {
            self.response.clone()
        }
    }

    #[test]
    fn empty_taxon_returns_none_qid() {
        let result = futures::executor::block_on(resolve(
            "",
            &StubRepo::ok(""),
            &mut SearchMetrics::default(),
        ));
        let r = result.unwrap();
        assert!(r.qid.is_none());
        assert_eq!(r.warnings.len(), 0);
    }

    #[test]
    fn star_taxon_returns_star() {
        let r = futures::executor::block_on(resolve(
            "*",
            &StubRepo::ok(""),
            &mut SearchMetrics::default(),
        ))
        .unwrap();
        assert_eq!(r.qid.as_deref(), Some("*"));
    }

    #[test]
    fn q_prefix_taxon_passes_through_uppercase() {
        let r = futures::executor::block_on(resolve(
            "q12345",
            &StubRepo::ok(""),
            &mut SearchMetrics::default(),
        ))
        .unwrap();
        assert_eq!(r.qid.as_deref(), Some("Q12345"));
    }

    #[test]
    fn remote_lookup_required_only_for_named_taxa() {
        assert!(!requires_remote_lookup(""));
        assert!(!requires_remote_lookup("*"));
        assert!(!requires_remote_lookup("Q12345"));
        assert!(!requires_remote_lookup("q12345"));
        assert!(requires_remote_lookup("Q"));
        assert!(requires_remote_lookup("q"));
        assert!(requires_remote_lookup("Gentiana lutea"));
    }

    #[test]
    fn network_error_becomes_transport_domain_error() {
        // Clear cache to ensure SPARQL path is taken.
        let result = futures::executor::block_on(resolve(
            "Completely Unknown Taxon XYZ Unique",
            &StubRepo::err_network("timeout"),
            &mut SearchMetrics::default(),
        ));
        assert!(
            matches!(
                result,
                Err(DomainError::Transport {
                    stage: QueryStage::TaxonSearch,
                    ..
                })
            ),
            "expected Transport error, got: {result:?}"
        );
    }

    #[test]
    fn empty_sparql_result_returns_taxon_not_found() {
        // CSV with only the header row → zero matches.
        let csv = "taxon,taxonLabel\n";
        let result = futures::executor::block_on(resolve(
            "Nonexistent Plant ABC",
            &StubRepo::ok(csv),
            &mut SearchMetrics::default(),
        ));
        assert!(
            matches!(
                result,
                Err(DomainError::Validation(
                    ValidationFault::TaxonNotFound { .. }
                ))
            ),
            "expected TaxonNotFound, got: {result:?}"
        );
    }

    #[test]
    fn a_repeat_search_reports_the_same_notice_as_the_first() {
        // The reported symptom: searching a name that both needed standardizing
        // and matched two candidates said one thing on the first run and another
        // on the second, because the second run read the answer from the cache
        // and the cache held only the QID.
        //
        // A name unique to this test, because the cache is process-wide.
        let csv = "taxon,taxon_name\nQ900001,Bacteriostaticum\nQ900002,Bacteriostaticum\n";
        let repo = StubRepo::ok(csv);

        let first = futures::executor::block_on(resolve(
            "bacteriostaticum",
            &repo,
            &mut SearchMetrics::default(),
        ))
        .expect("first run resolves");
        let second = futures::executor::block_on(resolve(
            "bacteriostaticum",
            &repo,
            &mut SearchMetrics::default(),
        ))
        .expect("second run resolves");

        assert_eq!(first.qid.as_deref(), Some("Q900001"));
        assert_eq!(first.warnings, second.warnings);
        assert_eq!(
            first.warnings,
            vec![
                LookupNotice::Standardized {
                    original: "bacteriostaticum".into(),
                    standardized: "Bacteriostaticum".into(),
                },
                LookupNotice::AmbiguousTaxon {
                    chosen_name: "Bacteriostaticum".into(),
                    chosen_qid: "Q900001".into(),
                    candidates: vec![
                        "Bacteriostaticum (Q900001)".into(),
                        "Bacteriostaticum (Q900002)".into(),
                    ],
                },
            ],
            "both notices, both times"
        );
    }
}
