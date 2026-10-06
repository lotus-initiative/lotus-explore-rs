// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `resolve_taxon`, in their own file.

#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
// `LotusRepository` is an async-fn-in-trait, so the stub's two methods must
// be `async fn`; a canned response has nothing to await, and the lint
// cannot tell the trait signature apart from a choice in the body.
#![expect(
    clippy::unused_async_trait_impl,
    reason = "LotusRepository is an AFIT: the `async fn` spelling is fixed by the trait"
)]

use super::*;
use crate::api::SearchResponse;
use crate::repositories::{LotusRepository, RepositoryError};
use lotus_search::SearchCriteria;

/// Stub that always returns a fixed SPARQL CSV response; API not configured.
#[derive(Clone)]
struct StubRepo {
    response: Result<lotus_search::ResponseBody, RepositoryError>,
}

impl StubRepo {
    fn ok(csv: &str) -> Self {
        Self {
            response: Ok(csv.as_bytes().to_vec().into()),
        }
    }
    fn err_network(msg: &str) -> Self {
        Self {
            response: Err(RepositoryError::network(msg)),
        }
    }
}

impl LotusRepository for StubRepo {
    async fn api_search(
        &self,
        _: &SearchCriteria,
        _: usize,
        _: bool,
    ) -> Option<Result<SearchResponse, RepositoryError>> {
        None
    }

    async fn sparql_body(&self, _: &str) -> Result<lotus_search::ResponseBody, RepositoryError> {
        self.response.clone()
    }
}

#[test]
fn empty_taxon_returns_none_qid() {
    let result = futures::executor::block_on(resolve(
        "",
        &StubRepo::ok(""),
        &mut SearchMetrics::default(),
    ));
    let r = result.unwrap();
    assert!(r.qid.is_none());
    assert_eq!(r.warnings.len(), 0);
}

#[test]
fn star_taxon_returns_star() {
    let r = futures::executor::block_on(resolve(
        "*",
        &StubRepo::ok(""),
        &mut SearchMetrics::default(),
    ))
    .unwrap();
    assert_eq!(r.qid.as_deref(), Some("*"));
}

#[test]
fn q_prefix_taxon_passes_through_uppercase() {
    let r = futures::executor::block_on(resolve(
        "q12345",
        &StubRepo::ok(""),
        &mut SearchMetrics::default(),
    ))
    .unwrap();
    assert_eq!(r.qid.as_deref(), Some("Q12345"));
}

#[test]
fn remote_lookup_required_only_for_named_taxa() {
    assert!(!requires_remote_lookup(""));
    assert!(!requires_remote_lookup("*"));
    assert!(!requires_remote_lookup("Q12345"));
    assert!(!requires_remote_lookup("q12345"));
    assert!(requires_remote_lookup("Q"));
    assert!(requires_remote_lookup("q"));
    assert!(requires_remote_lookup("Gentiana lutea"));
}

#[test]
fn network_error_becomes_transport_domain_error() {
    // Clear cache to ensure SPARQL path is taken.
    let result = futures::executor::block_on(resolve(
        "Completely Unknown Taxon XYZ Unique",
        &StubRepo::err_network("timeout"),
        &mut SearchMetrics::default(),
    ));
    assert!(
        matches!(
            result,
            Err(DomainError::Transport {
                stage: QueryStage::TaxonSearch,
                ..
            })
        ),
        "expected Transport error, got: {result:?}"
    );
}

#[test]
fn empty_sparql_result_returns_taxon_not_found() {
    // CSV with only the header row → zero matches.
    let csv = "taxon,taxonLabel\n";
    let result = futures::executor::block_on(resolve(
        "Nonexistent Plant ABC",
        &StubRepo::ok(csv),
        &mut SearchMetrics::default(),
    ));
    assert!(
        matches!(
            result,
            Err(DomainError::Validation(
                ValidationFault::TaxonNotFound { .. }
            ))
        ),
        "expected TaxonNotFound, got: {result:?}"
    );
}

#[test]
fn a_repeat_search_reports_the_same_notice_as_the_first() {
    // The reported symptom: a name that both needed standardizing and matched two
    // candidates said one thing on the first run and another on the second,
    // because the second read the answer from the cache, which held only the
    // QID.
    //
    // A name unique to this test: the cache is process-wide.
    let csv = "taxon,taxon_name\nQ900001,Bacteriostaticum\nQ900002,Bacteriostaticum\n";
    let repo = StubRepo::ok(csv);

    let first = futures::executor::block_on(resolve(
        "bacteriostaticum",
        &repo,
        &mut SearchMetrics::default(),
    ))
    .expect("first run resolves");
    let second = futures::executor::block_on(resolve(
        "bacteriostaticum",
        &repo,
        &mut SearchMetrics::default(),
    ))
    .expect("second run resolves");

    assert_eq!(first.qid.as_deref(), Some("Q900001"));
    assert_eq!(first.warnings, second.warnings);
    assert_eq!(
        first.warnings,
        vec![
            LookupNotice::Standardized {
                original: "bacteriostaticum".into(),
                standardized: "Bacteriostaticum".into(),
            },
            LookupNotice::AmbiguousTaxon {
                chosen_name: "Bacteriostaticum".into(),
                chosen_qid: "Q900001".into(),
                candidates: vec![
                    "Bacteriostaticum (Q900001)".into(),
                    "Bacteriostaticum (Q900002)".into(),
                ],
            },
        ],
        "both notices, both times"
    );
}
