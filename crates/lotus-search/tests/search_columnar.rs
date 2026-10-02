// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Searching the whole result set, streamed, with exact counts.
//!
//! The properties these pin are the three that separate this path from
//! [`lotus_search::search`]: no `LIMIT` in the query, no second `COUNT` query,
//! and a body that is never in memory all at once.

// The panic lints keep library code free of panics on external input. A test
// that fails on a bad fixture is reporting, not panicking.
#![allow(unused_crate_dependencies, clippy::expect_used, clippy::panic)]

use lotus_model::SearchCriteria;
use lotus_search::testing::Scripted;
use lotus_search::{SearchRequest, search_columnar};

const HEADER: &str = "compound,compoundLabel,compound_inchikey,compound_mass,compound_formula,taxon,taxon_name,ref_qid,statement\n";

fn payload() -> String {
    format!(
        "{HEADER}\
         Q1,first,,120.5,C2H6O,Q10,Homo,Q100,http://www.wikidata.org/entity/statement/Q1-0D8245CF-C1C0-45AA-8994-6BEBFF6B15EE\n\
         Q2,\"second, compound\",,300,C6H12O6,Q10,Homo,Q100,\n\
         Q1,first,,120.5,C2H6O,Q10,Homo,Q100,\n\
         Q3,third,,,,\n\
         Q4,fourth,,50,,Q11,Plant,Q200,\n"
    )
}

/// A transport scripted with the results query answered by `body`.
///
/// One reply, because an empty taxon input means "every taxon" and is answered
/// without a round trip. That is what makes `call_count` below a real assertion:
/// a count query would need a second one.
fn scripted(body: &str, chunk_size: usize) -> Scripted {
    Scripted::with_chunk_size(vec![(200, body)], chunk_size)
}

#[test]
fn the_query_carries_no_limit() {
    // The old path appended `LIMIT 500` server-side, which is why its filters
    // could only ever see 500 rows: the truncation happened before anything
    // downstream could undo it.
    let http = scripted(&payload(), 0);
    let result = block_on(search_columnar(
        &http,
        &SearchRequest::new(SearchCriteria::up_to_year(2025), 2025),
    ))
    .expect("the scripted transport answers");

    assert!(
        !result.query.contains("LIMIT"),
        "a limit would hide rows the filters have to see: {}",
        result.query
    );
}

#[test]
fn the_whole_set_arrives_and_its_counts_are_exact() {
    let http = scripted(&payload(), 0);
    let result = block_on(search_columnar(
        &http,
        &SearchRequest::new(SearchCriteria::up_to_year(2025), 2025),
    ))
    .expect("the scripted transport answers");
    let stats = result.set.stats();

    assert_eq!(result.set.row_count(), 5, "every row, duplicates included");
    assert_eq!(stats.n_entries, 5);
    assert_eq!(
        stats.n_entries_unique, 4,
        "rows one and three are the same triple"
    );
    assert_eq!(stats.n_compounds, 4);
    assert_eq!(stats.n_taxa, 2);
    assert_eq!(stats.n_references, 2);
}

#[test]
fn no_count_query_is_sent() {
    // The set holds every row and counts them itself, so the second query is
    // pure cost -- and it was the query whose two deleted blocks could silently
    // make a filter count nothing.
    let http = scripted(&payload(), 0);
    block_on(search_columnar(
        &http,
        &SearchRequest::new(SearchCriteria::up_to_year(2025), 2025),
    ))
    .expect("the scripted transport answers");

    assert_eq!(
        http.call_count(),
        1,
        "the results query, and nothing else: {:?}",
        http.queries()
    );
    assert!(
        !http.queries().iter().any(|q| q.contains("COUNT")),
        "no query should ask the endpoint to count: {:?}",
        http.queries()
    );
}

#[test]
fn the_result_is_the_same_however_the_body_arrives() {
    let whole = block_on(search_columnar(
        &scripted(&payload(), 0),
        &SearchRequest::new(SearchCriteria::up_to_year(2025), 2025),
    ))
    .expect("the scripted transport answers");

    // Every chunk size from one byte up, so the reader has to carry a record, a
    // quoted field and a multi-byte character across a boundary.
    for size in 1..=payload().len() {
        let streamed = block_on(search_columnar(
            &scripted(&payload(), size),
            &SearchRequest::new(SearchCriteria::up_to_year(2025), 2025),
        ))
        .expect("the scripted transport answers");

        assert_eq!(
            streamed.set.stats(),
            whole.set.stats(),
            "chunk size {size} changed the counts"
        );
        for row in 0..whole.set.row_count() {
            assert_eq!(
                streamed.set.entry(row),
                whole.set.entry(row),
                "chunk size {size} changed row {row}"
            );
        }
    }
}

#[test]
fn a_statement_comes_back_rebuilt_from_the_compound_and_its_uuid() {
    let http = scripted(&payload(), 3);
    let result = block_on(search_columnar(
        &http,
        &SearchRequest::new(SearchCriteria::up_to_year(2025), 2025),
    ))
    .expect("the scripted transport answers");

    assert_eq!(
        result.set.statement_text(0).as_deref(),
        Some("Q1-0D8245CF-C1C0-45AA-8994-6BEBFF6B15EE"),
        "the 41-byte URI prefix and the compound prefix are both re-derived"
    );
    assert_eq!(result.set.statement_text(1), None, "no statement, no cell");
}

#[test]
fn a_row_with_no_taxon_does_not_count_as_a_taxon() {
    let http = scripted(&payload(), 0);
    let result = block_on(search_columnar(
        &http,
        &SearchRequest::new(SearchCriteria::up_to_year(2025), 2025),
    ))
    .expect("the scripted transport answers");

    assert_eq!(
        result.set.taxon_qid(3),
        None,
        "row three is the one with no taxon"
    );
    assert_eq!(
        result.set.stats().n_taxa,
        2,
        "the two real taxa, and not the absent one"
    );
}

/// Drive a future to completion on the current thread.
///
/// The search is a single async fn with no reactor requirement beyond the
/// transport's, and the test double has no timers, so a hand-rolled block-on is
/// enough -- and it keeps the test from needing an executor this crate does not
/// already depend on for wasm.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, Waker};

    let mut context = Context::from_waker(Waker::noop());
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::hint::spin_loop(),
        }
    }
}
