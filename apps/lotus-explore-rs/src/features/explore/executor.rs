// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Search execution pipeline for the Explore feature.

// The search future is not `Send`, and cannot be made so without changing what
// the code is. `on_phase` is a closure over a Dioxus `Signal`, which is a
// `RefCell` and therefore not `Sync`; the closure is held across an `.await`, so
// the future it is part of is not `Send`.
//
// The fix is not a bound. Adding `Send + Sync` to the callback is refused by the
// compiler at the one place that constructs it, which is the evidence that the
// bound is wrong rather than missing. Making it work means not capturing the
// signal in the callback -- passing the phase out and updating it on the caller's
// side -- which is a change to how search progress is reported, not a lint fix.
//
// So the allow stands, and it stands here for every module on this path. The
// eighteen others are the same cause reached from different directions.
//
// This lint is in `nursery`, which the workspace denies. It is worth keeping the
// deny and this one allow: the other eighteen-eighteen sites that are genuinely
// fixable are what the rest of the deny is for.
#![allow(clippy::future_not_send)]

use crate::features::explore::outcome::SearchOutcome;
use crate::features::explore::request::SearchRequest;
use crate::features::explore::search_metrics::{SearchMetrics, emit_search_summary};
use crate::features::explore::service::{
    api_pipeline, build_query::normalize_smiles, results_pipeline, strategy::ExecutionStrategy,
};
use crate::features::explore::types::{DomainError, QueryPhase};
use crate::perf;
use crate::repositories::LotusRepository;
use crate::services::search_telemetry as telemetry;

pub struct SearchExecutor<R, P>
where
    R: LotusRepository,
    P: Fn(QueryPhase),
{
    repo: R,
    on_phase: P,
}

impl<R, P> SearchExecutor<R, P>
where
    R: LotusRepository,
    P: Fn(QueryPhase),
{
    #[must_use]
    pub const fn new(repo: R, on_phase: P) -> Self {
        Self { repo, on_phase }
    }

    pub async fn execute(&self, request: &SearchRequest) -> Result<SearchOutcome, DomainError> {
        let search_timer = perf::start_timer("LOTUS:search_total");
        let mut metrics = SearchMetrics::default();
        telemetry::search_start();

        // The REST API fast-path is opt-in: it only runs when the user has
        // explicitly enabled it (non-empty base URL). Otherwise we go direct to
        // SPARQL so a misconfigured/absent API never wastes a request on every
        // search (and never triggers the "builder error" fallback storm).
        let api_enabled = crate::api::api_base_url().is_some_and(|b| !b.is_empty());
        let strategy = ExecutionStrategy::resolve(request.direct_download(), api_enabled);
        let smiles = normalize_smiles(&request.criteria().structure);

        match strategy {
            ExecutionStrategy::ApiFirst => {
                if let Some(outcome) =
                    api_pipeline::try_execute(request, &smiles, &self.repo, &mut metrics).await
                {
                    let total_elapsed = perf::end_timer("LOTUS:search_total", search_timer);
                    emit_search_summary(total_elapsed, metrics);
                    return Ok(outcome);
                }
            }
            ExecutionStrategy::Direct => {
                telemetry::api_path_not_available("reason=api_disabled");
            }
            ExecutionStrategy::DownloadOnly => {
                telemetry::api_path_not_available("reason=download_only_mode");
            }
        }

        let pipeline_outcome = match results_pipeline::execute(
            request,
            &smiles,
            &self.repo,
            &mut metrics,
            &self.on_phase,
            strategy.is_download_only(),
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(error) => {
                let _ = perf::end_timer("LOTUS:search_total", search_timer);
                return Err(error);
            }
        };

        if strategy.is_download_only() {
            let total_elapsed = perf::end_timer("LOTUS:search_total", search_timer);
            telemetry::direct_download_ready(total_elapsed);
            emit_search_summary(total_elapsed, metrics);
            return Ok(SearchOutcome::from_results_pipeline(pipeline_outcome));
        }

        let outcome = SearchOutcome::from_results_pipeline(pipeline_outcome);

        let total_elapsed = perf::end_timer("LOTUS:search_total", search_timer);
        telemetry::search_complete(
            total_elapsed,
            outcome.rows.len(),
            outcome.total_matches.unwrap_or(outcome.rows.len()),
        );
        emit_search_summary(total_elapsed, metrics);
        Ok(outcome)
    }
}

/// Execute the full search pipeline; returns a [`SearchOutcome`] or a
/// [`DomainError`]. No locale strings are produced here.
pub async fn do_search<R: LotusRepository>(
    request: &SearchRequest,
    repo: R,
    on_phase: impl Fn(QueryPhase),
) -> Result<SearchOutcome, DomainError> {
    SearchExecutor::new(repo, on_phase).execute(request).await
}
