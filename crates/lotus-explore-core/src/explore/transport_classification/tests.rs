// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `transport_classification`, in their own file.

use super::*;

#[test]
fn classifies_http_syntax_error_as_query_syntax() {
    let kind = classify_transport_error(&RepositoryError::Http {
        status: 400,
        body: "Invalid SPARQL query: Token \"AS\": mismatched input 'AS' expecting ','".to_string(),
    });
    assert_eq!(kind, TransportFailureKind::QuerySyntax);
}

#[test]
fn classifies_http_5xx_without_known_signature_as_server() {
    let kind = classify_transport_error(&RepositoryError::Http {
        status: 503,
        body: "service unavailable".to_string(),
    });
    assert_eq!(kind, TransportFailureKind::Server);
}

#[test]
fn classifies_parse_cache_conflict_as_cache_conflict() {
    let kind = classify_transport_error(&RepositoryError::parse(
        "Trying to insert a cache key which was already present",
    ));
    assert_eq!(kind, TransportFailureKind::CacheConflict);
}

#[test]
fn a_429_that_says_the_query_timed_out_is_not_a_rate_limit() {
    // Both arrive as 429 and mean opposite things. QLever's own answer,
    // measured:
    //
    //   429 {"exception":"Operation timed out. Last operation: Sort ... on ?r"}
    //
    // is one query outstaying the endpoint's budget; treating it as a busy
    // endpoint is what made one broad search cost four full-length queries.
    let cancelled = classify_transport_error(&RepositoryError::Http {
        status: 429,
        body: "Operation timed out. Last operation: Sort (internal order) on ?r".to_string(),
    });
    assert_eq!(cancelled, TransportFailureKind::QueryTooExpensive);
    assert!(
        !cancelled.is_retryable(),
        "the same query sent again will be cancelled the same way"
    );

    // A proxy in front of the endpoint can still answer 429 for a request
    // rate, and that one is worth coming back to.
    let refused = classify_transport_error(&RepositoryError::Http {
        status: 429,
        body: "Too many requests from this client".to_string(),
    });
    assert_eq!(refused, TransportFailureKind::RateLimit);
}

#[test]
fn configuration_and_network_have_expected_retryability() {
    assert!(!TransportFailureKind::Configuration.is_retryable());
    assert!(TransportFailureKind::Network.is_retryable());
}

/// "timeout" in a 4xx body is not a rate limit. `QLever` answers a request whose
/// time budget exceeds its ceiling with a 403 that says so; the same request
/// gets the same 403 however often it is sent, so it must not be retried.
/// A 4xx that does say "too many" or "rate limit" still is one.
#[test]
fn a_client_error_mentioning_timeout_is_not_a_rate_limit() {
    let ceiling = RepositoryError::Http {
        status: 403,
        body: "User submitted timeout was higher than what is currently allowed by this instance (30s)."
            .into(),
    };
    let kind = classify_transport_error(&ceiling);
    assert_eq!(kind, TransportFailureKind::BadRequest);
    assert!(!kind.is_retryable());

    let throttled = RepositoryError::Http {
        status: 403,
        body: "Too many requests from this client".into(),
    };
    assert_eq!(
        classify_transport_error(&throttled),
        TransportFailureKind::RateLimit
    );
}
