// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Fetching the whole result set in the browser.
//!
//! One query, no row limit, and a body that is folded into a columnar set a chunk
//! at a time. The three things this replaced are each gone for a reason:
//!
//! - **`LIMIT`.** It was appended server-side, so the table's filters could only
//!   ever see the first 500 rows of the result set. Nothing downstream could undo
//!   it; the truncation happened before the request left.
//! - **The `COUNT` query.** The set holds every row and deduplicates nothing, so
//!   its statistics are the counts. That also retires `counts_query`, whose
//!   construction deletes two named blocks from the query text and whose filter
//!   re-binding can silently make a filter count nothing.
//! - **The response cache of CSV bodies.** It held eight whole payloads with no
//!   bound on body size. What is cached now is the finished set, and an `Arc` of
//!   it costs nothing to share, so there is one copy rather than eight.

use super::{FetchResult, PlannedResultsFetch};
use super::{PROGRESS_ROW_STEP, ProgressThrottle};
use crate::features::explore::search_metrics::SearchMetrics;
use crate::features::explore::types::{DomainError, ParseFault, QueryStage};
use crate::perf;
use crate::repositories::LotusRepository;
use crate::repositories::RepositoryError;
use crate::services::search_telemetry as telemetry;
use std::sync::Arc;

pub(super) async fn fetch_results<R: LotusRepository>(
    repo: &R,
    plan: &PlannedResultsFetch<'_>,
    metrics: &mut SearchMetrics,
    on_processing: &impl Fn(),
    on_progress: &mut impl FnMut(usize),
) -> Result<FetchResult, DomainError> {
    // Reuse a previously built set on back/repeat navigation instead of
    // re-hitting QLever. The key is the query text, which is now the whole
    // request: there is no limit left to distinguish one page from another.
    let key = plan.execution_query.to_owned();

    let timer = perf::start_timer("LOTUS:results_columnar_query");
    let (set, fetched_remote) = if let Some(cached) = crate::cache::take_cached_set(&key) {
        (cached, false)
    } else {
        let mut throttle = ProgressThrottle::new(PROGRESS_ROW_STEP);
        let mut progress = |p: lotus_search::StreamProgress| {
            if let Some(rows) = throttle.offer(p.rows) {
                on_progress(rows);
            }
        };
        let fetched = repo
            .sparql_columnar(plan.execution_query, &mut progress)
            .await
            .map_err(DomainError::transport_at(QueryStage::ResultsQuery))?;
        // The last chunk is the one that matters, and it can fall short of the step.
        if let Some(rows) = throttle.finish() {
            on_progress(rows);
        }
        let set = Arc::new(fetched);
        crate::cache::store_cached_set(key, Arc::clone(&set));
        (set, true)
    };
    let elapsed = perf::end_timer("LOTUS:results_columnar_query", timer);
    if fetched_remote {
        metrics.add_network(elapsed);
    }

    on_processing();

    let stats = set.stats();
    telemetry::results_fetch_done(elapsed, set.row_count(), stats.n_entries);
    Ok(FetchResult::from_set(set))
}

pub(super) fn is_probable_memory_limit(err: &DomainError) -> bool {
    fn has_memory_signature(msg: &str) -> bool {
        let m = msg.to_ascii_lowercase();
        m.contains("out of memory")
            || m.contains("memory")
            || m.contains("too large")
            || m.contains("allocation")
            || m.contains("capacity")
    }

    match err {
        DomainError::Transport { source, .. } => match source {
            // Neither of these is a body too big to hold, so neither should earn
            // the reader a "narrow your query" hint.
            //
            // `Truncated` is the one worth spelling out: a stalled or cut-short
            // body is a *transfer* failure, and offering to narrow the query for
            // it would fix it only by accident -- by accident returning fewer rows
            // quickly. It is grouped with `NotConfigured` because that is what the
            // question being asked here answers for both: is there evidence the
            // result set did not fit?
            RepositoryError::NotConfigured | RepositoryError::Truncated(_) => false,
            RepositoryError::Network(detail) | RepositoryError::Parse(detail) => {
                has_memory_signature(detail.as_ref())
            }
            RepositoryError::Http { body, .. } => has_memory_signature(body),
        },
        DomainError::Parse(ParseFault::ResultsCsv { details }) => has_memory_signature(details),
        _ => false,
    }
}
