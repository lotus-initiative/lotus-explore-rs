// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![allow(clippy::future_not_send)]
//! SPARQL-side results pipeline for Explore searches.
//! Owns the non-API execution path after strategy selection:
//! More detail in the type and function docs below.

use super::fetch_results;
mod plan;

use crate::features::explore::request::SearchRequest;
use crate::features::explore::search_metrics::SearchMetrics;
use crate::features::explore::types::{DomainError, QueryPhase, TaxonWarning};
use crate::models::runtime_table_row_limit;
use crate::models::{CompoundEntry, DatasetStats};
use crate::repositories::LotusRepository;

#[derive(Debug)]
pub struct ResultsPipelineOutcome {
    pub rows: Vec<CompoundEntry>,
    pub qid: Option<String>,
    pub warning: Option<TaxonWarning>,
    pub query: String,
    pub total_matches: Option<usize>,
    pub total_stats: Option<DatasetStats>,
    pub display_capped_rows: bool,
    pub endpoint: crate::export::SparqlEndpoint,
}

pub async fn execute<R: LotusRepository>(
    request: &SearchRequest,
    normalized_smiles: &str,
    repo: &R,
    metrics: &mut SearchMetrics,
    on_phase: impl Fn(QueryPhase),
    direct_download_mode: bool,
) -> Result<ResultsPipelineOutcome, DomainError> {
    let plan =
        plan::build_execution_plan(request, normalized_smiles, repo, metrics, &on_phase).await?;

    if direct_download_mode {
        return Ok(plan.into_download_only_outcome());
    }

    let display_limit = runtime_table_row_limit();
    let fetch_result = fetch_results::fetch(
        plan.execution_query(),
        display_limit,
        repo,
        metrics,
        fetch_results::FetchHooks::new(
            || on_phase(QueryPhase::FetchingResults),
            || on_phase(QueryPhase::ProcessingResults),
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
    use crate::models::SearchCriteria;
    use crate::repositories::mock::MockRepository;

    #[test]
    fn download_only_builds_query_without_fetching_results() {
        futures::executor::block_on(async {
            let request = SearchRequest::new(
                SearchCriteria {
                    taxon: String::new(),
                    structure: String::new(),
                    ..SearchCriteria::up_to_year(crate::models::current_year())
                },
                SearchCommand::StartupDownload,
            );
            let repo = MockRepository::sparql_error("should not fetch rows");
            let mut metrics = SearchMetrics::default();

            let outcome = execute(&request, "", &repo, &mut metrics, |_| {}, true)
                .await
                .expect("download-only should not hit results fetch");

            assert!(outcome.rows.is_empty());
            assert!(outcome.total_matches.is_none());
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
                    ..SearchCriteria::up_to_year(crate::models::current_year())
                },
                SearchCommand::Interactive,
            );
            let repo = MockRepository::sparql_only(
                b"compound,compoundLabel,taxon,ref_qid\nQ1,One,Q10,Q20\nQ2,Two,Q11,Q21\n".to_vec(),
            );
            let mut metrics = SearchMetrics::default();

            let outcome = execute(&request, "", &repo, &mut metrics, |_| {}, false)
                .await
                .expect("interactive pipeline should fetch results");

            assert_eq!(outcome.rows.len(), 2);
            assert_eq!(outcome.total_matches, Some(2));
            assert!(outcome.total_stats.is_some());
        });
    }
}
