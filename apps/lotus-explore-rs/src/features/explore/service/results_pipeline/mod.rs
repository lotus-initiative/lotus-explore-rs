// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
// The search future is not `Send`: `on_phase` is a closure over a Dioxus
// `Signal`, which is a `RefCell`. See the full explanation in
// `features/explore/executor.rs`, which is the same cause reached from here.
#![expect(
    clippy::future_not_send,
    reason = "`on_phase` captures a Dioxus `Signal`, which is a `RefCell` and not `Sync`"
)]
//! SPARQL-side results pipeline: the non-API execution path, after strategy
//! selection.

use super::fetch_results;
mod plan;

use crate::features::explore::request::SearchRequest;
use crate::features::explore::search_metrics::SearchMetrics;
use crate::features::explore::types::{DomainError, LookupNotice, QueryPhase};
use crate::repositories::LotusRepository;

#[derive(Debug)]
pub struct ResultsPipelineOutcome {
    pub set: std::sync::Arc<lotus_model::ColumnarResultSet>,
    pub qid: Option<String>,
    pub warnings: Vec<LookupNotice>,
    pub query: String,
    pub display_capped_rows: bool,
    pub endpoint: crate::export::SparqlEndpoint,
}

pub async fn execute<R: LotusRepository>(
    request: &SearchRequest,
    normalized_smiles: &str,
    repo: &R,
    metrics: &mut SearchMetrics,
    on_phase: impl Fn(QueryPhase),
    on_progress: impl Fn(usize),
    direct_download_mode: bool,
) -> Result<ResultsPipelineOutcome, DomainError> {
    let plan =
        plan::build_execution_plan(request, normalized_smiles, repo, metrics, &on_phase).await?;

    if direct_download_mode {
        return Ok(plan.into_download_only_outcome());
    }

    let fetch_result = fetch_results::fetch(
        plan.execution_query(),
        repo,
        metrics,
        fetch_results::FetchHooks::new(
            || on_phase(QueryPhase::FetchingResults),
            || on_phase(QueryPhase::ProcessingResults),
            on_progress,
        ),
    )
    .await?;

    Ok(plan.into_interactive_outcome(fetch_result))
}

#[cfg(test)]
mod tests;
