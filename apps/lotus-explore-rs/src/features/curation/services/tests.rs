// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `services`, in their own file.

#![allow(clippy::expect_used)]

use super::*;
use crate::sparql::{QLEVER_WIKIDATA, WDQS_SCHOLARLY, WDQS_WIKIDATA};
use futures::executor::block_on;
use std::sync::{Arc, Mutex};

fn run_with_mock(
    query: &str,
    first_result: Result<String, FetchError>,
) -> (Vec<(String, &'static str)>, Result<String, FetchError>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let calls_for_executor = Arc::clone(&calls);
    let result = block_on(execute_sparql_with_wdqs_fallback_with(
        query,
        ResponseFormat::SparqlJson,
        move |query, endpoint, _| {
            let call_index = calls_for_executor.lock().expect("calls lock").len();
            calls_for_executor
                .lock()
                .expect("calls lock")
                .push((query.to_owned(), endpoint));
            let response = if call_index == 0 {
                first_result.clone()
            } else {
                Ok("ok".to_owned())
            };
            Box::pin(async move { response })
        },
    ));
    let recorded = calls.lock().expect("calls lock").clone();
    (recorded, result)
}

#[test]
fn qlever_success_never_uses_wdqs() {
    let query = "SELECT ?ref WHERE { ?ref wdt:P356 \"10.1/x\" }";
    let (calls, result) = run_with_mock(query, Ok("qlever".to_owned()));

    assert!(result.is_ok());
    assert_eq!(calls, vec![(query.to_owned(), QLEVER_WIKIDATA)]);
}

#[test]
fn qlever_network_failure_falls_back() {
    let query = "SELECT ?item WHERE { ?item wdt:P31 wd:Q16521 }";
    let (calls, result) = run_with_mock(
        query,
        Err(FetchError::Network("connection failed".to_owned())),
    );

    assert!(result.is_ok());
    assert_eq!(calls.len(), 2);
    assert_eq!(calls.first().expect("QLever call").1, QLEVER_WIKIDATA);
    assert_eq!(calls.get(1).expect("fallback call").1, WDQS_WIKIDATA);
}

#[test]
fn qlever_502_falls_back_by_query_kind() {
    let cases = [
        (
            "SELECT ?item WHERE { ?item wdt:P31 wd:Q16521 }",
            WDQS_WIKIDATA,
        ),
        (
            "SELECT ?ref WHERE { ?ref wdt:P356 \"10.1/x\" }",
            WDQS_SCHOLARLY,
        ),
    ];

    for (query, expected_endpoint) in cases {
        let gateway = FetchError::Http {
            status: 502,
            message: "bad gateway".to_owned(),
        };
        let (calls, result) = run_with_mock(query, Err(gateway));

        assert!(result.is_ok());
        assert_eq!(calls.len(), 2);
        let first = calls.first().expect("QLever call");
        let second = calls.get(1).expect("fallback call");
        assert_eq!(first.1, QLEVER_WIKIDATA);
        assert_eq!(second.1, expected_endpoint);
        assert_eq!(first.0, query);
        assert_eq!(second.0, query);
    }
}
