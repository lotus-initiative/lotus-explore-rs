// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `error`, in their own file.

use super::*;

#[test]
fn a_rejected_query_is_not_retried_but_a_failing_endpoint_is() {
    let rejected = FetchError::Http {
        status: 400,
        message: "bad".into(),
    };
    assert!(!rejected.is_retryable());
    assert!(
        FetchError::Http {
            status: 503,
            message: "down".into()
        }
        .is_retryable()
    );
    assert!(FetchError::Network("timeout".into()).is_retryable());
}

#[test]
fn a_query_that_ran_out_of_time_is_not_retried() {
    // The load-bearing assertion of this change, and the one a future edit is
    // most likely to undo by "helpfully" putting 429 back into the retryable
    // set. QLever's 429 is a per-query time limit, so the retry does not
    // make the query succeed -- it makes the endpoint spend its whole budget
    // again, once per attempt, on a query already known to be too big.
    let cancelled = FetchError::TimedOut {
        budget: Some("30s".into()),
        message: "Operation timed out. Last operation: Sort (internal order) on ?r".into(),
    };
    assert!(!cancelled.is_retryable());
    assert!(!is_retryable_status(429));
    assert!(cancelled.is_timed_out());
    // It is a 429 on the wire, so everything above this crate that reads
    // statuses keeps working.
    assert_eq!(cancelled.status(), Some(429));
    // And it is not the endpoint being unreachable: WDQS would run the same
    // expensive query a second time, on a second endpoint.
    assert!(!cancelled.is_endpoint_unavailable());
}

#[test]
fn a_result_set_cut_short_is_not_retried() {
    // The load-bearing assertion. Retrying a body that stopped half way
    // re-transfers the hundreds of megabytes that already arrived in order
    // to arrive at the same place, so `is_retryable` returning true here is
    // what makes a slow query slow rather than fast.
    let stalled = FetchError::Truncated {
        bytes_read: 512_000_000,
        reason: TruncationReason::Stalled,
    };
    assert!(!stalled.is_retryable());
    assert!(stalled.is_truncated());
    assert!(
        !FetchError::Truncated {
            bytes_read: 0,
            reason: TruncationReason::ClosedEarly,
        }
        .is_retryable()
    );
}

#[test]
fn a_cut_short_result_set_is_not_the_endpoint_being_gone() {
    // This one decides whether the query is re-sent to WDQS. Truncation says
    // nothing about reachability, so routing on it would double a transfer
    // that was already too large for one.
    let cut = FetchError::Truncated {
        bytes_read: 1,
        reason: TruncationReason::ClosedEarly,
    };
    assert!(!cut.is_endpoint_unavailable());
}

#[test]
fn a_cut_short_result_set_carries_no_http_status() {
    // The CSV parsed and no status line ever came back mid-body, so anything
    // that branches on the status must see `None` rather than a guess.
    assert_eq!(
        FetchError::Truncated {
            bytes_read: 42,
            reason: TruncationReason::Stalled,
        }
        .status(),
        None
    );
}

#[test]
fn the_message_says_the_answer_was_incomplete() {
    let message = FetchError::Truncated {
        bytes_read: 1_024,
        reason: TruncationReason::ClosedEarly,
    }
    .to_string();
    assert!(message.contains("1024"), "{message}");
    assert!(message.contains("cut short"), "{message}");
}

#[test]
fn only_a_gateway_error_means_the_endpoint_is_gone() {
    // A 500 means the query broke the server, and WDQS would answer the
    // same way. A 502 means the front door did not open.
    assert!(
        FetchError::Http {
            status: 502,
            message: String::new()
        }
        .is_endpoint_unavailable()
    );
    assert!(FetchError::Network("refused".into()).is_endpoint_unavailable());
    assert!(
        !FetchError::Http {
            status: 500,
            message: String::new()
        }
        .is_endpoint_unavailable()
    );
    assert!(
        !FetchError::Http {
            status: 400,
            message: String::new()
        }
        .is_endpoint_unavailable()
    );
}

#[test]
fn the_status_is_available_for_logging_and_for_the_ui() {
    assert_eq!(
        FetchError::Http {
            status: 429,
            message: String::new()
        }
        .status(),
        Some(429)
    );
    assert_eq!(FetchError::Network("x".into()).status(), None);
}

/// The rule itself, on its own, rather than only through an error.
///
/// These call the free function directly. The tests above went through
/// `FetchError::is_retryable`, a *separate* implementation of the same rule -- the
/// duplication that let every mutant of `is_retryable_status` survive, since nothing
/// reached the function they were mutating. It is now the only copy, and these tests
/// hold it to that.
#[test]
fn the_retry_rule_is_a_server_error_and_nothing_else() {
    // The client-error side: none of them is worth another attempt. A 400 is
    // the endpoint saying no, 404 will not become a 200 on a second try, and
    // 429 is QLever cancelling a query that ran out of time -- which a
    // second attempt reproduces exactly, at full cost.
    for status in [
        200, 201, 204, 301, 400, 401, 403, 404, 409, 418, 428, 430, 451,
    ] {
        assert!(
            !is_retryable_status(status),
            "{status} is a settled answer and retrying it wastes an attempt"
        );
    }

    // 429 was the one client error this used to retry, on the reading that
    // it meant "too many requests, come back later". QLever uses it for a
    // per-query time limit instead, so this is the assertion that keeps the
    // change from being undone.
    assert!(
        !is_retryable_status(429),
        "a cancelled query must not be re-sent"
    );

    // The boundary. 500 is the first server error and 599 the last; a server
    // error anywhere in between is retryable, and 499 is a client error.
    assert!(is_retryable_status(500), "500 is the first server error");
    assert!(is_retryable_status(503), "503 is retryable");
    assert!(is_retryable_status(599), "599 is still a server error");
    assert!(
        !is_retryable_status(499),
        "499 is below the boundary and is not a server error"
    );

    // There is deliberately no upper bound: the rule is "500 or above", not
    // "500 through 599". An HTTP status is three digits, so nothing above 599
    // can arrive from a real endpoint, and capping the range would add a
    // comparison to a hot check to reject a status that cannot exist.
    assert!(
        is_retryable_status(u16::MAX),
        "the rule has no upper bound, and that is intentional"
    );
}

/// The error's answer and the free function's answer are the same answer.
///
/// This is the assertion that keeps the two from drifting again, which is the
/// bug the free function was introduced to prevent.
#[test]
fn the_error_and_the_rule_agree() {
    for status in [200, 400, 404, 429, 499, 500, 502, 503, 599] {
        let through_the_error = FetchError::Http {
            status,
            message: String::new(),
        }
        .is_retryable();
        assert_eq!(
            through_the_error,
            is_retryable_status(status),
            "status {status}: the error and the rule disagree"
        );
    }
}

/// Every format asks for what it says it wants.
///
/// The `Accept` header and the `action=` parameter are the whole of how a format reaches the
/// endpoint, so a constant in either place is silent: the request succeeds and returns
/// the wrong shape, which the parser reports as somebody else's bug. `NTriples` has no
/// `action` at all, a fact about `QLever` rather than an oversight -- pinning it stops a
/// future arm "fixing" it into a fourth action.
#[test]
fn every_format_names_its_own_accept_header_and_action() {
    let cases = [
        (ResponseFormat::Csv, "text/csv", Some("csv_export")),
        (
            ResponseFormat::SparqlJson,
            "application/sparql-results+json",
            Some("sparql_json_export"),
        ),
        (ResponseFormat::Turtle, "text/turtle", Some("turtle_export")),
        (ResponseFormat::NTriples, "application/n-triples", None),
    ];

    for (format, accept, action) in cases {
        assert_eq!(format.accept(), accept, "{format:?} accept header");
        assert_eq!(format.qlever_action(), action, "{format:?} qlever action");
    }

    // The headers have to be distinct: two formats sharing one would make the
    // endpoint's answer depend on which arm matched first.
    let mut accepts: Vec<&str> = cases.iter().map(|(f, _, _)| f.accept()).collect();
    accepts.sort_unstable();
    accepts.dedup();
    assert_eq!(
        accepts.len(),
        cases.len(),
        "two formats share an Accept header"
    );
}
