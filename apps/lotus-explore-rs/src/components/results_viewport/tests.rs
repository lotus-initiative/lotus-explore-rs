// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `results_viewport`, in their own file.

// A gate failing on a missing anchor is reporting, not panicking on input.
#![allow(clippy::expect_used)]

/// The query of a failed search is behind a disclosure, not in the open.
///
/// It used to be an always-expanded `<pre>`, which made the largest element on
/// the page the least useful one: a reader whose search failed was looking at a
/// wall of SPARQL while the reason it failed sat in a banner above it.
///
/// Read as source, like the other gates in this crate, because the rendered
/// output of a `#[component]` is not available to a host test run.
#[test]
fn the_failed_query_is_not_shown_expanded() {
    const SOURCE: &str = include_str!("../results_viewport.rs");

    assert!(
        SOURCE.contains("fn QueryDisplay"),
        "the disclosure has to exist for this gate to mean anything"
    );
    let display = SOURCE
        .split_once("fn QueryDisplay")
        .expect("QueryDisplay is defined")
        .1;

    assert!(
        display.contains("details {"),
        "a failed query goes behind a native disclosure, matching the SPARQL \
         panel in the results toolbar"
    );
    assert!(
        !display.contains("open: true"),
        "the disclosure starts closed: a reader who did not ask for the query \
         should not be handed it"
    );
    assert!(
        !display.contains("h2"),
        "the query is no longer the page's heading; the warning leads instead"
    );
}

/// The warning is in the summary, so it is visible without expanding anything.
///
/// A summary that has to be opened to find out whether the search worked is the
/// wrong summary: the reason for the failure is in the banner above, and this
/// says why there is more to see.
#[test]
fn the_summary_carries_the_warning_and_not_a_neutral_label() {
    const SOURCE: &str = include_str!("../results_viewport.rs");
    let display = SOURCE
        .split_once("fn QueryDisplay")
        .expect("QueryDisplay is defined")
        .1;

    let summary = display
        .split_once("summary {")
        .expect("the disclosure has a summary")
        .1;
    assert!(
        summary.contains("TextKey::ShowFailedQuery"),
        "the summary is the warning; it cannot be the generic SPARQL label"
    );
    assert!(
        !summary.contains("TextKey::SparqlQuery"),
        "a neutral label says nothing about why a search failed"
    );
}

/// What is shown is a reconstruction, and says so.
///
/// `dispatch_error` rebuilds the query from the criteria rather than remembering
/// what was sent, so calling it the query that failed would overstate it.
#[test]
fn the_reconstruction_is_disclosed_rather_than_claimed() {
    const SOURCE: &str = include_str!("../results_viewport.rs");
    let display = SOURCE
        .split_once("fn QueryDisplay")
        .expect("QueryDisplay is defined")
        .1;

    assert!(
        display.contains("TextKey::FailedQueryReconstructed"),
        "the query is rebuilt from the criteria, and the reader is told so"
    );
}
