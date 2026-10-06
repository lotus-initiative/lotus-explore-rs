// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `results_pipeline`, in their own file.

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
