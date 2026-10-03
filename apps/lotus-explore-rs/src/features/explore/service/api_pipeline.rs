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

            // Only usable if what came back *is* the whole result set.
            //
            // The API answers with a page plus the true total. The columnar set
            // derives its counts from the rows it holds, so a partial page would
            // report the page's row count as the total -- understating the result
            // by whatever was left behind. There is no honest way to keep both:
            // a total that disagrees with the rows is the bug this whole path
            // exists to remove. So a partial page is declined and the caller falls
            // through to the streaming SPARQL path, which returns everything.
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
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::panic)]
    // `LotusRepository` is an async-fn-in-trait, so the stub's two methods must
    // be `async fn`; a canned response has nothing to await, and the lint
    // cannot tell the trait signature apart from a choice in the body.
    #![expect(
        clippy::unused_async_trait_impl,
        reason = "LotusRepository is an AFIT: the `async fn` spelling is fixed by the trait"
    )]

    use super::*;
    use crate::api::SearchResponse;
    use crate::features::explore::command::SearchCommand;
    use lotus_search::SearchCriteria;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Clone)]
    struct StubRepo {
        api_result: Rc<RefCell<Option<Result<SearchResponse, RepositoryError>>>>,
        seen_criteria: Rc<RefCell<Option<SearchCriteria>>>,
    }

    impl StubRepo {
        fn successful(response: SearchResponse) -> Self {
            Self {
                api_result: Rc::new(RefCell::new(Some(Ok(response)))),
                seen_criteria: Rc::new(RefCell::new(None)),
            }
        }

        fn not_configured() -> Self {
            Self {
                api_result: Rc::new(RefCell::new(Some(Err(RepositoryError::NotConfigured)))),
                seen_criteria: Rc::new(RefCell::new(None)),
            }
        }
    }

    impl LotusRepository for StubRepo {
        async fn api_search(
            &self,
            criteria: &SearchCriteria,
            _: usize,
            _: bool,
        ) -> Option<Result<SearchResponse, RepositoryError>> {
            *self.seen_criteria.borrow_mut() = Some(criteria.clone());
            self.api_result.borrow_mut().take()
        }

        async fn sparql_body(
            &self,
            _: &str,
        ) -> Result<lotus_search::ResponseBody, RepositoryError> {
            panic!("api fast-path tests should not hit SPARQL")
        }
    }

    /// A response whose `total_matches` matches the rows it carries, which is
    /// the only shape this path will accept.
    fn complete_response() -> SearchResponse {
        let mut response = sample_response();
        response.total_matches = response.rows.len();
        response
    }

    /// A response that is a *page*: one row against a total of three.
    fn sample_response() -> SearchResponse {
        serde_json::from_value(serde_json::json!({
            "resolved_taxon_qid": "Q123",
            "warning": "normalized from API",
            "query": "SELECT * WHERE { ?compound ?p ?o }",
            "rows": [{
                "compound_qid": "Q1",
                "name": "Alpha",
                "inchikey": null,
                "smiles": "C",
                "mass": 10.0,
                "formula": "CH4",
                "taxon_qid": "QTaxon",
                "taxon_name": "Rosa",
                "reference_qid": "QRef",
                "ref_title": "Paper",
                "ref_doi": null,
                "pub_year": 2020,
                "statement": null
            }],
            "total_matches": 3,
            "stats": {
                "n_compounds": 1,
                "n_taxa": 1,
                "n_references": 1,
                "n_entries": 3,
                "n_entries_unique": 3
            }
        }))
        .expect("valid search response JSON")
    }

    #[test]
    fn successful_api_path_normalizes_smiles_and_builds_search_outcome() {
        futures::executor::block_on(async {
            let repo = StubRepo::successful(complete_response());
            let request = SearchRequest::new(
                SearchCriteria {
                    taxon: "Rosa".into(),
                    structure: "raw smiles should be replaced".into(),
                    ..SearchCriteria::up_to_year(crate::clock::current_year())
                },
                SearchCommand::Interactive,
            );
            let mut metrics = SearchMetrics::default();

            let outcome = try_execute(&request, "C1=CC=CC=C1", &repo, &mut metrics)
                .await
                .expect("api response should short-circuit search");

            assert_eq!(
                repo.seen_criteria.borrow().as_ref().unwrap().structure,
                "C1=CC=CC=C1"
            );
            assert_eq!(outcome.set.row_count(), 1);
            assert_eq!(outcome.qid.as_deref(), Some("Q123"));
            assert_eq!(
                outcome.set.stats().n_entries,
                outcome.set.row_count(),
                "the counts are the rows that arrived"
            );
            assert!(
                !outcome.display_capped_rows,
                "nothing was truncated, so nothing can be reported as truncated"
            );
            assert_eq!(metrics.sparql_calls, 1);
        });
    }

    #[test]
    fn a_partial_api_page_is_declined_so_the_counts_cannot_understate() {
        // The API answers with a page plus the true total. Folding only the page
        // into a columnar set would report the page's row count as the total, so
        // the caller falls through to the streaming SPARQL path instead. This is
        // the one behaviour change on this path, and it is the whole point of the
        // refactor: a total that disagrees with the rows is not a trade worth
        // making for one fewer round trip.
        futures::executor::block_on(async {
            let repo = StubRepo::successful(sample_response());
            let request = SearchRequest::new(
                SearchCriteria {
                    taxon: "Rosa".into(),
                    ..SearchCriteria::up_to_year(crate::clock::current_year())
                },
                SearchCommand::Interactive,
            );
            let mut metrics = SearchMetrics::default();

            let outcome = try_execute(&request, "C1=CC=CC=C1", &repo, &mut metrics).await;

            assert!(
                outcome.is_none(),
                "a page of 1 row against a total of 3 is not a result set"
            );
        });
    }

    #[test]
    fn not_configured_api_path_falls_through_without_outcome() {
        futures::executor::block_on(async {
            let repo = StubRepo::not_configured();
            let request = SearchRequest::new(
                SearchCriteria::up_to_year(crate::clock::current_year()),
                SearchCommand::Interactive,
            );
            let mut metrics = SearchMetrics::default();

            let outcome = try_execute(&request, "", &repo, &mut metrics).await;

            assert!(outcome.is_none());
            assert_eq!(metrics.sparql_calls, 0);
        });
    }
}
