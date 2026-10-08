// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `results_pipeline`, in their own file.

#![allow(clippy::expect_used)]

use super::*;
use crate::features::explore::ValidationFault;
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
fn a_reference_that_is_not_an_identifier_is_refused_not_dropped() {
    // `resolve_reference::resolve` raises `ReferenceNotAnIdentifier` for input that is
    // neither a QID nor a DOI, and its own test says the point is "the reader is told
    // the accepted forms rather than having the constraint dropped in silence". The
    // pipeline did not do that: it wrapped the call in `requires_remote_lookup`, which
    // is false for exactly this input, so the validator never ran and the reference was
    // blanked. The reader got every compound that matched the rest of their search and
    // no indication the reference field had been ignored.
    //
    // The taxon field one function above has the right shape already -- `resolve` is
    // called unconditionally and the guard only decides whether to show a phase.
    for input in ["Gentiana lutea", "10.10/x", "Anti-inflammatory activity"] {
        futures::executor::block_on(async {
            let request = SearchRequest::new(
                SearchCriteria {
                    taxon: String::new(),
                    structure: String::new(),
                    reference: input.to_string(),
                    ..SearchCriteria::up_to_year(crate::clock::current_year())
                },
                SearchCommand::Interactive,
            );
            let repo = MockRepository::sparql_only(Vec::new());
            let mut metrics = SearchMetrics::default();

            let outcome = execute(&request, "", &repo, &mut metrics, |_| {}, |_| {}, false).await;

            assert!(
                matches!(
                    outcome,
                    Err(DomainError::Validation(
                        ValidationFault::ReferenceNotAnIdentifier { .. }
                    ))
                ),
                "{input:?} should be refused rather than silently dropped, got {outcome:?}"
            );
        });
    }
}

#[test]
fn an_empty_reference_stays_a_no_constraint_and_asks_nothing() {
    // The other half of the same guard: a blank field must not cost a query, which is
    // what `requires_remote_lookup` was there for.
    futures::executor::block_on(async {
        let request = SearchRequest::new(
            SearchCriteria {
                taxon: String::new(),
                structure: String::new(),
                reference: "   ".to_string(),
                ..SearchCriteria::up_to_year(crate::clock::current_year())
            },
            SearchCommand::Interactive,
        );
        let repo = MockRepository::sparql_only(
            b"compound,compoundLabel,taxon,ref_qid\nQ1,One,Q10,Q20\n".to_vec(),
        );
        let mut metrics = SearchMetrics::default();

        let outcome = execute(&request, "", &repo, &mut metrics, |_| {}, |_| {}, false)
            .await
            .expect("a blank reference is not a constraint");

        assert_eq!(outcome.set.row_count(), 1);
        assert!(
            !outcome.query.contains("VALUES ?r"),
            "a blank reference must bind nothing: {}",
            outcome.query
        );
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
