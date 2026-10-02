// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
// The search future is not `Send`: `on_phase` is a closure over a Dioxus
// `Signal`, which is a `RefCell`. See the full explanation in
// `features/explore/executor.rs`, which is the same cause reached from here.
#![expect(
    clippy::future_not_send,
    reason = "`on_phase` captures a Dioxus `Signal`, which is a `RefCell` and not `Sync`"
)]
//! Full-results fetch service.
//! This module is **Dioxus-free**: the phase-change callback (`on_fetching`)
//! is a plain `Fn()` closure so that tests can supply a no-op.

use crate::features::explore::search_metrics::SearchMetrics;
use crate::features::explore::types::DomainError;
#[cfg(target_arch = "wasm32")]
use crate::features::explore::types::QueryStage;
use crate::repositories::LotusRepository;
use crate::services::search_telemetry as telemetry;
use lotus_model::{CompoundEntry, DatasetStats};

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod wasm;

/// Result of a successful full-results fetch.
pub struct FetchResult {
    pub rows: Vec<CompoundEntry>,
    pub total_stats: Option<DatasetStats>,
    pub total_matches: Option<usize>,
    pub display_capped_rows: bool,
}

impl FetchResult {
    /// A search that was never sent, so nothing came back.
    ///
    /// Download-only searches resolve a taxon and build a query without fetching
    /// anything, and their outcome differs from a real search only in this.
    pub const fn empty() -> Self {
        Self {
            rows: Vec::new(),
            total_stats: None,
            total_matches: None,
            display_capped_rows: false,
        }
    }
}

pub struct FetchHooks<OnFetching, OnProcessing> {
    on_fetching: OnFetching,
    on_processing: OnProcessing,
}

impl<OnFetching, OnProcessing> FetchHooks<OnFetching, OnProcessing> {
    pub const fn new(on_fetching: OnFetching, on_processing: OnProcessing) -> Self {
        Self {
            on_fetching,
            on_processing,
        }
    }
}

struct PlannedResultsFetch<'a> {
    execution_query: &'a str,
    display_limit: usize,
}

/// Whether the rows fetched are the whole result set.
///
/// A page that came back shorter than the budget cannot be hiding anything, so
/// the count is known without asking anyone. This is the common case by a wide
/// margin — most searches match fewer rows than the table's ceiling — and it is
/// what lets a small search cost one query instead of two.
///
/// A page that came back *full* is ambiguous: it may be everything, or the first
/// `display_limit` of something larger. Only the count query can tell those
/// apart, so this is the one case where it earns its cost.
#[must_use]
pub(super) fn page_is_whole_result_set(rows_fetched: usize, display_limit: usize) -> bool {
    rows_fetched < display_limit
}

/// Whether the table is showing fewer rows than matched.
///
/// Known without the endpoint when the page is the whole result set, which is
/// most searches. When the page was full it is only known if the count came back
/// and exceeded what was fetched — and a full page whose count failed counts as
/// capped, because that is what it might be, and saying "not capped" on no
/// evidence would hide the truncation from the reader.
#[must_use]
pub(super) fn display_is_capped(rows: usize, display_limit: usize, total: Option<usize>) -> bool {
    if page_is_whole_result_set(rows, display_limit) {
        return false;
    }
    total.is_none_or(|t| t > rows)
}

/// Fetch full results with a single query and cap rendered rows locally.
/// `on_fetching` is called before the network fetch begins and `on_processing`
/// before CSV parsing/stat aggregation; in tests pass `|| ()`.
pub(super) async fn fetch<R: LotusRepository, OnFetching: Fn(), OnProcessing: Fn()>(
    execution_query: &str,
    display_limit: usize,
    repo: &R,
    metrics: &mut SearchMetrics,
    hooks: FetchHooks<OnFetching, OnProcessing>,
) -> Result<FetchResult, DomainError> {
    // Clear any previous WDQS fallback state from taxon resolution or other operations
    crate::repositories::reset_wdqs_fallback_flag();

    let plan = plan_full_results_fetch(execution_query, display_limit);
    let FetchHooks {
        on_fetching,
        on_processing,
    } = hooks;
    on_fetching();
    telemetry::results_fetch_started(plan.display_limit);

    #[cfg(target_arch = "wasm32")]
    let result = wasm::fetch_results(repo, &plan, metrics, &on_processing).await;

    #[cfg(not(target_arch = "wasm32"))]
    let result = native::fetch_results(repo, &plan, metrics, &on_processing).await;

    match result {
        Ok(v) => Ok(v),
        Err(err) => {
            #[cfg(target_arch = "wasm32")]
            {
                // Keep previous wasm classification semantics for memory pressure errors.
                if wasm::is_probable_memory_limit(&err) {
                    return Err(DomainError::memory_limit(QueryStage::ResultsQuery));
                }
                Err(err)
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                Err(err)
            }
        }
    }
}

const fn plan_full_results_fetch(
    execution_query: &str,
    display_limit: usize,
) -> PlannedResultsFetch<'_> {
    PlannedResultsFetch {
        execution_query,
        display_limit,
    }
}

#[cfg(test)]
mod paging_tests {
    use super::{display_is_capped, page_is_whole_result_set};

    #[test]
    fn a_page_that_came_back_short_is_the_whole_result_set() {
        // The case that makes a small search cost one query rather than two: an
        // `InChIKey` that resolves to a compound with a few dozen rows comes
        // back against a 500 budget, and there is nothing left to fetch.
        assert!(page_is_whole_result_set(0, 500));
        assert!(page_is_whole_result_set(20, 500));
        assert!(page_is_whole_result_set(499, 500));
    }

    #[test]
    fn a_page_that_came_back_full_might_not_be_the_whole_result_set() {
        // Exactly `display_limit` rows is the ambiguous case, and the reason the
        // count query still exists at all. Testing `>=` here instead of `>` is
        // the whole difference between a correct answer and one that silently
        // under-reports on every full page.
        assert!(!page_is_whole_result_set(500, 500));
        assert!(!page_is_whole_result_set(501, 500));
    }

    #[test]
    fn a_short_page_is_never_reported_as_capped() {
        // Including when a total is present and larger, which cannot happen from
        // `from_entries` but is the shape a bug would take.
        assert!(!display_is_capped(20, 500, Some(20)));
        assert!(!display_is_capped(20, 500, Some(500)));
        assert!(!display_is_capped(20, 500, None));
    }

    #[test]
    fn a_full_page_is_capped_only_when_the_count_says_there_is_more() {
        assert!(
            display_is_capped(500, 500, Some(12_000)),
            "12,000 matched and 500 are shown"
        );
        assert!(
            !display_is_capped(500, 500, Some(500)),
            "500 matched and 500 are shown: the page is everything"
        );
    }

    #[test]
    fn a_full_page_whose_count_failed_is_reported_as_capped() {
        // The honest answer. A full page might be truncated, and nothing came
        // back to say it is not, so claiming completeness would hide the
        // truncation from the reader.
        assert!(display_is_capped(500, 500, None));
    }
}
