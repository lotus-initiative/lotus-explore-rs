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
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::features::explore::command::SearchCommand;
    use crate::features::explore::request::SearchRequest;
    use crate::repositories::mock::MockRepository;
    use lotus_search::SearchCriteria;

    #[test]
    fn download_only_builds_query_without_fetching_results() {
        futures::executor::block_on(async {
            let request = SearchRequest::new(
                SearchCriteria {
                    taxon: String::new(),
                    structure: String::new(),
                    ..SearchCriteria::up_to_year(crate::clock::current_year())
                },
                SearchCommand::StartupDownload,
            );
            let repo = MockRepository::sparql_error("should not fetch rows");
            let mut metrics = SearchMetrics::default();

            let outcome = execute(&request, "", &repo, &mut metrics, |_| {}, |_| {}, true)
                .await
                .expect("download-only should not hit results fetch");

            assert_eq!(outcome.set.row_count(), 0, "expected no entries");
            assert_eq!(outcome.set.row_count(), 0, "nothing was fetched");
            assert!(outcome.query.contains("SELECT"));
        });
    }

    #[test]
    fn interactive_pipeline_fetches_rows_and_counts() {
        futures::executor::block_on(async {
            let request = SearchRequest::new(
                SearchCriteria {
                    taxon: String::new(),
                    structure: String::new(),
                    ..SearchCriteria::up_to_year(crate::clock::current_year())
                },
                SearchCommand::Interactive,
            );
            let repo = MockRepository::sparql_only(
                b"compound,compoundLabel,taxon,ref_qid\nQ1,One,Q10,Q20\nQ2,Two,Q11,Q21\n".to_vec(),
            );
            let mut metrics = SearchMetrics::default();

            let outcome = execute(&request, "", &repo, &mut metrics, |_| {}, |_| {}, false)
                .await
                .expect("interactive pipeline should fetch results");

            assert_eq!(outcome.set.row_count(), 2);
            assert_eq!(
                outcome.set.stats().n_entries,
                2,
                "the set carries the count"
            );
            assert_eq!(outcome.set.stats().n_entries_unique, 2);
        });
    }
}
