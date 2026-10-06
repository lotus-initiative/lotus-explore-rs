// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Which query a failure came from.
//!
//! A search runs three queries in sequence -- the taxon lookup, the structure or
//! reference lookup, and the results -- and they fail for different reasons. The
//! `stage` on `SearchError::Transport` is the only thing that tells a reader
//! which one, and three of the four values had no test, so a stage constant
//! pasted wrong would have been indistinguishable from no stage at all.

// The panic lints keep library code free of panics on external input. A test
// failing on a bad script is reporting, not panicking.
#![allow(unused_crate_dependencies)]
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use lotus_model::SearchCriteria;
use lotus_search::testing::Scripted;
use lotus_search::{SearchError, SearchRequest, search, search_columnar};

const NOW: u16 = 2026;

/// A criteria with no taxon, no structure and no reference: `search` then runs
/// exactly one query, the results one.
const fn bare() -> SearchCriteria {
    SearchCriteria::up_to_year(NOW)
}

/// A transport whose every reply is "the request never arrived", repeated so a
/// retry loop and the `WDQS` fallback both find one waiting.
///
/// A status of 0 is not a rejection, and it is what makes this exercise the
/// fallback path: `is_endpoint_unavailable` is true for it, so `QLever` failing
/// sends the query to `WDQS`, which fails the same way.
fn always_unreachable() -> Scripted {
    let http = Scripted::new(vec![(0, "never arrived")]);
    http.then_always_from(0, 0, "never arrived");
    http
}

/// The stage a failure is reported against, or `""` if it is not a transport
/// error.
const fn stage_of(error: &SearchError) -> &str {
    match error {
        SearchError::Transport { stage, .. } => stage,
        SearchError::Invalid(_) => "",
    }
}

#[tokio::test]
async fn a_taxon_lookup_failure_is_reported_against_the_taxon_query() {
    let mut criteria = bare();
    criteria.taxon = "Gentiana lutea".to_string();
    let http = always_unreachable();

    let error = search(&http, &SearchRequest::new(criteria, NOW))
        .await
        .expect_err("an unreachable endpoint fails the search");
    assert_eq!(
        stage_of(&error),
        "taxon",
        "the first query to run is the taxon lookup, so that is the stage"
    );
}

#[tokio::test]
async fn a_reference_lookup_failure_is_reported_against_the_second_query() {
    // The taxon is answered locally from an empty box, so the reference lookup
    // is the first thing to touch the network.
    let mut criteria = bare();
    criteria.reference = "10.1000/xyz".to_string();
    let http = always_unreachable();

    let error = search(&http, &SearchRequest::new(criteria, NOW))
        .await
        .expect_err("an unreachable endpoint fails the search");
    assert_eq!(
        stage_of(&error),
        "structure or reference",
        "the taxon box was empty, so the DOI lookup is what failed"
    );
}

#[tokio::test]
async fn a_results_failure_is_reported_against_the_results_query() {
    // Taxon and reference both resolve without a round trip here, so the only
    // thing left to fail is the results query -- and it is reported as such
    // rather than inheriting an earlier stage's name.
    let http = always_unreachable();

    let error = search(&http, &SearchRequest::new(bare(), NOW))
        .await
        .expect_err("an unreachable endpoint fails the search");
    assert_eq!(
        stage_of(&error),
        "results",
        "with nothing to resolve, the results query is the one that ran"
    );
}

#[tokio::test]
async fn the_streaming_search_reports_the_same_stages() {
    // Two search functions, one set of stages. If only one of them were
    // annotated, the caller would see a stage it cannot act on for half the
    // searches it runs.
    let http = always_unreachable();
    let error = search_columnar(&http, &SearchRequest::new(bare(), NOW))
        .await
        .expect_err("an unreachable endpoint fails the search");
    assert_eq!(
        stage_of(&error),
        "results",
        "the streaming path has the same three stages as the buffered one"
    );

    let mut criteria = bare();
    criteria.taxon = "Gentiana lutea".to_string();
    let http = always_unreachable();
    let error = search_columnar(&http, &SearchRequest::new(criteria, NOW))
        .await
        .expect_err("an unreachable endpoint fails the search");
    assert_eq!(
        stage_of(&error),
        "taxon",
        "including the taxon stage, before any result is streamed"
    );
}

#[tokio::test]
async fn a_stage_is_named_in_the_message_rather_than_only_in_the_variant() {
    // The stage reaches a human through `Display`, not through a pattern match
    // on the enum -- the app renders the message. A stage that exists in the
    // variant and not in the text is a stage nobody is told.
    let mut criteria = bare();
    criteria.taxon = "Gentiana lutea".to_string();
    let http = always_unreachable();

    let error = search(&http, &SearchRequest::new(criteria, NOW))
        .await
        .expect_err("an unreachable endpoint fails the search");
    let rendered = error.to_string();
    assert!(
        rendered.contains("taxon") && rendered.contains("failed"),
        "the message must name the query that failed: {rendered:?}"
    );
}
