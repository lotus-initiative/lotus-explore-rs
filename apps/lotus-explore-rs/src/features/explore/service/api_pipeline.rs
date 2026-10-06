// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
// The search future is not `Send`: `on_phase` is a closure over a Dioxus
// `Signal`, which is a `RefCell`. See the full explanation in
// `features/explore/executor.rs`, which is the same cause reached from here.
#![expect(
    clippy::future_not_send,
    reason = "`on_phase` captures a Dioxus `Signal`, which is a `RefCell` and not `Sync`"
)]
//! REST API fast-path execution for Explore searches.

use crate::features::explore::outcome::SearchOutcome;
use crate::features::explore::request::SearchRequest;
use crate::features::explore::search_metrics::SearchMetrics;
use crate::perf;
use crate::repositories::{LotusRepository, RepositoryError};
use crate::services::search_telemetry as telemetry;
use crate::table_budget::API_MAX_ROWS;

pub async fn try_execute<R: LotusRepository>(
    request: &SearchRequest,
    normalized_smiles: &str,
    repo: &R,
    metrics: &mut SearchMetrics,
) -> Option<SearchOutcome> {
    let mut api_criteria = request.criteria().clone();
    api_criteria.structure.clear();
    api_criteria.structure.push_str(normalized_smiles);
    let display_limit = API_MAX_ROWS;
    let api_timer = perf::start_timer("LOTUS:api_search");

    match repo.api_search(&api_criteria, display_limit, true).await {
        None | Some(Err(RepositoryError::NotConfigured)) => {
            let _ = perf::end_timer("LOTUS:api_search", api_timer);
            telemetry::api_path_not_available("reason=not_configured");
            None
        }
        Some(Err(err)) => {
            let api_elapsed = perf::end_timer("LOTUS:api_search", api_timer);
            telemetry::api_fallback_direct(api_elapsed, &err.to_string());
            None
        }
        Some(Ok(response)) => {
            let api_elapsed = perf::end_timer("LOTUS:api_search", api_timer);

            // Only usable if what came back *is* the whole result set. The columnar set
            // derives its counts from the rows it holds, so a partial page would
            // report the page's row count as the total. A total that disagrees
            // with the rows is the bug this whole path exists to remove, so a
            // partial page is declined and the caller falls through to the
            // streaming SPARQL path.
            // The timer is already closed above; ending it again would ask the
            // browser to close a label that is not open.
            if response.total_matches > response.rows.len() {
                telemetry::api_fallback_direct(api_elapsed, "reason=api_returned_a_partial_page");
                return None;
            }

            metrics.add_network(api_elapsed);
            telemetry::record_api_success(api_elapsed, response.rows.len(), response.total_matches);
            Some(SearchOutcome::from_api_response(response))
        }
    }
}

#[cfg(test)]
#[path = "api_pipeline/tests.rs"]
mod tests;
