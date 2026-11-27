// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `hybrid`, in their own file.

use super::*;

#[test]
fn strip_limit_removes_trailing_limit() {
    let q = "SELECT ?s WHERE { ?s ?p ?o } LIMIT 500";
    assert_eq!(strip_limit_clause(q), "SELECT ?s WHERE { ?s ?p ?o }");
}

#[test]
fn strip_limit_preserves_query_without_limit() {
    let q = "SELECT ?s WHERE { ?s ?p ?o }";
    assert_eq!(strip_limit_clause(q), "SELECT ?s WHERE { ?s ?p ?o }");
}

#[test]
fn strip_limit_preserves_inner_limit() {
    let q = "SELECT ?s WHERE { { SELECT ?s WHERE { ?s ?p ?o } LIMIT 10 } LIMIT 500";
    // Should only strip the last LIMIT
    assert_eq!(
        strip_limit_clause(q),
        "SELECT ?s WHERE { { SELECT ?s WHERE { ?s ?p ?o } LIMIT 10 }"
    );
}

#[test]
fn strip_limit_handles_case_insensitive() {
    let q = "SELECT ?s WHERE { ?s ?p ?o } limit 500";
    assert_eq!(strip_limit_clause(q), "SELECT ?s WHERE { ?s ?p ?o }");
}

#[test]
fn strip_limit_handles_newline() {
    let q = "SELECT ?s WHERE { ?s ?p ?o }\nLIMIT 500";
    assert_eq!(strip_limit_clause(q), "SELECT ?s WHERE { ?s ?p ?o }");
}

#[test]
fn map_fetch_error_preserves_http_status_and_body() {
    let mapped = map_fetch_error(FetchError::Http {
        status: 400,
        message: "invalid query".to_string(),
    });
    assert_eq!(
        mapped,
        RepositoryError::Http {
            status: 400,
            body: "invalid query".to_string(),
        }
    );
}

#[test]
fn map_fetch_error_keeps_network_as_network() {
    let mapped = map_fetch_error(FetchError::Network("timeout".to_string()));
    assert!(matches!(mapped, RepositoryError::Network(_)));
}

#[test]
fn qlever_unavailability_includes_502_and_network_failures() {
    let gateway = FetchError::Http {
        status: 502,
        message: "upstream gateway error (HTML payload)".into(),
    };
    assert!(gateway.is_endpoint_unavailable());
    assert!(FetchError::Network("connection failed".into()).is_endpoint_unavailable());
    assert!(
        !FetchError::Http {
            status: 500,
            message: "boom".into()
        }
        .is_endpoint_unavailable()
    );
    assert!(
        !FetchError::Http {
            status: 400,
            message: "bad query".into()
        }
        .is_endpoint_unavailable()
    );
    assert!(!FetchError::Empty.is_endpoint_unavailable());
}

#[test]
fn needs_split_occurrences_only_for_unconstrained_and_broad_clades() {
    assert!(
        needs_split_occurrences(&lotus_query::all_compounds_query()),
        "wildcard * query must use split occurrences"
    );
    assert!(
        needs_split_occurrences(&lotus_query::all_compounds_including_untaxonomised_query()),
        "empty-taxon whole-graph query must use split occurrences"
    );
    assert!(
        needs_split_occurrences(&lotus_query::compounds_by_taxon_query("Q756")),
        "Plantae (Q756) must use split occurrences"
    );
    assert!(
        needs_split_occurrences(&lotus_query::compounds_by_taxon_query("Q21754")),
        "Gentianales (Q21754) must use split occurrences"
    );
    assert!(
        !needs_split_occurrences(&lotus_query::compounds_by_taxon_query("Q7560")),
        "Q7560 must not prefix-match Q756"
    );
    assert!(
        !needs_split_occurrences(&lotus_query::compounds_by_taxon_query("Q158572")),
        "species/genus/order searches like Gentiana lutea (Q158572) run in a single POST"
    );

    let mut filtered = SearchCriteria::up_to_year(2025);
    filtered.taxon = "Q756".to_string();
    filtered.mass_min = 100.0;
    filtered.mass_max = 250.0;
    let with_mass = lotus_query::with_filters(
        &lotus_query::compounds_by_taxon_query("Q756"),
        &filtered,
        2025,
    );
    assert!(
        !needs_split_occurrences(&with_mass),
        "narrowed Plantae search with mass filter runs in a single POST"
    );
}

#[test]
fn timeout_and_truncation_trigger_split_fallback_but_rate_limit_does_not() {
    assert!(is_timeout_or_truncation(&RepositoryError::Http {
        status: 429,
        body: "the query exceeded the endpoint's time limit (30s): Operation timed out".into(),
    }));
    assert!(is_timeout_or_truncation(&RepositoryError::truncated(
        "cut short"
    )));
    assert!(
        !is_timeout_or_truncation(&RepositoryError::Http {
            status: 429,
            body: "429 Too Many Requests".into(),
        }),
        "an nginx rate-limit refusal must not immediately fire 3 more requests via split fallback"
    );
}

#[test]
fn rate_limit_refusal_distinguishes_proxy_429_from_query_timeout() {
    assert!(
        is_rate_limit_refusal(&FetchError::TimedOut {
            budget: Some("30s".into()),
            message: "429 Too Many Requests".into(),
        }),
        "nginx 429 Too Many Requests must be retried in-place with backoff"
    );
    assert!(
        is_rate_limit_refusal(&FetchError::Http {
            status: 429,
            message: "Too Many Requests".into(),
        }),
        "HTTP 429 rate-limit refusal must be retried in-place with backoff"
    );
    assert!(
        !is_rate_limit_refusal(&FetchError::TimedOut {
            budget: Some("30s".into()),
            message: "Operation timed out. Last operation: Sort on ?r".into(),
        }),
        "QLever 30s query timeout must never be retried as a rate limit"
    );
    assert!(
        !is_rate_limit_refusal(&FetchError::TimedOut {
            budget: None,
            message: "Query exceeded its deadline.".into(),
        }),
        "WDQS v2 504 timeout must never be retried as a rate limit"
    );
    assert!(
        !is_rate_limit_refusal(&FetchError::Http {
            status: 500,
            message: "Internal Server Error".into(),
        }),
        "non-429 status is not a rate-limit refusal"
    );
}

#[test]
fn prepare_wdqs_fallback_query_routes_scholarly_and_main_queries() {
    reset_wdqs_fallback_flag();
    let doi_query = lotus_query::reference_by_doi_query("10.1000/xyz");
    let (endpoint, rewritten) = prepare_wdqs_fallback_query(&doi_query);
    assert_eq!(endpoint, lotus_model::WDQS_SCHOLARLY);
    assert_eq!(rewritten, doi_query);

    reset_wdqs_fallback_flag();
    let taxon_query = lotus_query::compounds_by_taxon_query("Q16521");
    let (endpoint, rewritten) = prepare_wdqs_fallback_query(&taxon_query);
    assert_eq!(endpoint, lotus_model::WDQS_WIKIDATA);
    assert!(
        rewritten.contains("SERVICE <https://query-scholarly-next.wikidata.org/sparql>"),
        "{rewritten}"
    );
    reset_wdqs_fallback_flag();
}

/// A query that ran out of time must reach the retry planner as too expensive,
/// whatever the endpoint wrote in its message: `WDQS`'s is the echoed query, not
/// `QLever`'s "Operation timed out". Read as a rate limit, it was retried at the
/// full time budget each time.
#[test]
fn a_timed_out_query_is_too_expensive_whatever_its_message() {
    for message in ["Operation timed out", "SPARQL-QUERY: queryStr=SELECT ?x"] {
        let mapped = map_fetch_error(lotus_search::FetchError::TimedOut {
            budget: None,
            message: message.into(),
        });
        let kind = lotus_explore_core::explore::transport_classification::classify_transport_error(
            &mapped,
        );
        assert_eq!(
            kind,
            lotus_explore_core::explore::transport_classification::TransportFailureKind::QueryTooExpensive,
            "{message}"
        );
    }
}
