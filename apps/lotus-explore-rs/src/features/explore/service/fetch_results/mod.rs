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
use lotus_model::ColumnarResultSet;
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod wasm;

/// Result of a successful full-results fetch.
///
/// Just the result set. There is no row limit anywhere on this path and no second
/// query for the totals, so there is nothing else to carry: the set holds every
/// row the endpoint returned, deduplicates nothing, and
/// [`ColumnarResultSet::stats`] *is* the answer the `COUNT` query used to give.
///
/// A field here could disagree with the rows it describes. That was the bug this
/// path exists to fix, so the struct is not given the chance to reintroduce it.
pub struct FetchResult {
    /// Every row, stored by column.
    pub set: Arc<ColumnarResultSet>,
    /// Always false: nothing is truncated, because nothing was capped.
    pub display_capped_rows: bool,
}

impl FetchResult {
    /// A search that was never sent, so nothing came back.
    ///
    /// Download-only searches resolve a taxon and build a query without fetching
    /// anything, and their outcome differs from a real search only in this.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            set: Arc::new(ColumnarResultSet::default()),
            display_capped_rows: false,
        }
    }

    /// A set, and nothing else.
    #[must_use]
    pub fn from_set(set: Arc<ColumnarResultSet>) -> Self {
        Self {
            display_capped_rows: false,
            set,
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
}

/// Fetch the whole result set with a single query.
///
/// `on_fetching` is called before the network fetch begins and `on_processing`
/// before the body is folded into the set; in tests pass `|| ()`.
pub(super) async fn fetch<R: LotusRepository, OnFetching: Fn(), OnProcessing: Fn()>(
    execution_query: &str,
    repo: &R,
    metrics: &mut SearchMetrics,
    hooks: FetchHooks<OnFetching, OnProcessing>,
) -> Result<FetchResult, DomainError> {
    // Clear any previous WDQS fallback state from taxon resolution or other operations
    crate::repositories::reset_wdqs_fallback_flag();

    let plan = plan_full_results_fetch(execution_query);
    let FetchHooks {
        on_fetching,
        on_processing,
    } = hooks;
    on_fetching();
    telemetry::results_fetch_started();

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

const fn plan_full_results_fetch(execution_query: &str) -> PlannedResultsFetch<'_> {
    PlannedResultsFetch { execution_query }
}

#[cfg(test)]
mod no_limit_gate {
    //! The interactive search must not put a row limit back on the query.
    //!
    //! A source-level check, because the file it guards is `wasm`-only. What it
    //! pins is that `limit_query` is not reachable from the browser fetch path: a
    //! `LIMIT` is appended server-side, so nothing downstream can undo it, and the
    //! table's filters would go back to seeing only the rows inside it while the
    //! toolbar reported the whole set's count.
    //!
    //! The tests look for the identifier *followed by a parenthesis*, which is a
    //! call. Matching the bare name would also match this module's own prose -- the
    //! file being checked explains in a comment why `counts_query` is gone -- and a
    //! gate that fails when someone documents the thing it forbids is a gate that
    //! gets deleted.

    /// The wasm fetch module, as text. Compiled here only to be read.
    const WASM_FETCH: &str = include_str!("wasm.rs");

    #[test]
    fn the_fetch_path_never_limits_the_query() {
        assert!(
            !WASM_FETCH.contains("limit_query("),
            "a server-side LIMIT truncates the result before anything downstream \
             can see it, which is the bug this whole path exists to remove"
        );
        assert!(
            WASM_FETCH.contains("sparql_columnar"),
            "the browser fetches the whole set, streamed into a columnar store"
        );
    }

    #[test]
    fn there_is_still_no_count_query() {
        assert!(
            !WASM_FETCH.contains("counts_query("),
            "the set carries its own exact counts, so a second query is pure cost"
        );
    }
}
