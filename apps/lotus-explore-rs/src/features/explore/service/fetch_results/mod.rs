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
