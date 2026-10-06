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
/// Just the result set. No row limit anywhere on this path and no second query for
/// totals, so there is nothing else to carry: the set holds every row the endpoint
/// returned, deduplicates nothing, and [`ColumnarResultSet::stats`] *is* the answer
/// the `COUNT` query used to give.
///
/// A field here could disagree with the rows it describes — the bug this path
/// exists to fix, so the struct is not given the chance to reintroduce it.
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
    /// anything; their outcome differs from a real search only in this.
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

/// Callbacks the fetch makes into the UI, as a type rather than as parameters.
///
/// Three positional closures would be three `impl Fn` bounds in a row, and the
/// order would be the only thing telling them apart.
mod progress;

#[cfg(target_arch = "wasm32")]
pub use progress::{PROGRESS_ROW_STEP, ProgressThrottle};

pub struct FetchHooks<OnFetching, OnProcessing, OnProgress> {
    fetching: OnFetching,
    processing: OnProcessing,
    /// Rows received so far, called while the body is still arriving.
    ///
    /// Already throttled by the fetcher: this is not called per chunk.
    progress: OnProgress,
}

impl<OnFetching, OnProcessing, OnProgress> FetchHooks<OnFetching, OnProcessing, OnProgress> {
    pub const fn new(fetching: OnFetching, processing: OnProcessing, progress: OnProgress) -> Self {
        Self {
            fetching,
            processing,
            progress,
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
// `progress` is only called on the wasm path, so only there does it need to be
// mutable; otherwise the native build warns about a `mut` it cannot use.
#[cfg_attr(
    not(target_arch = "wasm32"),
    expect(
        unused_mut,
        reason = "the progress callback is only called on the wasm path"
    )
)]
pub(super) async fn fetch<
    R: LotusRepository,
    OnFetching: Fn(),
    OnProcessing: Fn(),
    OnProgress: FnMut(usize),
>(
    execution_query: &str,
    repo: &R,
    metrics: &mut SearchMetrics,
    hooks: FetchHooks<OnFetching, OnProcessing, OnProgress>,
) -> Result<FetchResult, DomainError> {
    // Clear any previous WDQS fallback state from taxon resolution or other operations
    crate::repositories::reset_wdqs_fallback_flag();

    let plan = plan_full_results_fetch(execution_query);
    let FetchHooks {
        fetching,
        processing,
        mut progress,
    } = hooks;
    fetching();
    telemetry::results_fetch_started();

    #[cfg(target_arch = "wasm32")]
    let result = wasm::fetch_results(repo, &plan, metrics, &processing, &mut progress).await;

    // The native path assembles the export on disk with no chunked reader to count
    // rows in, so there is no progress to report. Dropped explicitly, because
    // "no progress on this path" is a decision and accidental silence would look
    // identical.
    #[cfg(not(target_arch = "wasm32"))]
    let result = {
        drop(progress);
        native::fetch_results(repo, &plan, metrics, &processing).await
    };

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
mod no_limit_gate;
