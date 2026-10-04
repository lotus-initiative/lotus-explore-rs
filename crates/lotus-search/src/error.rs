// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use thiserror::Error;

/// A failed request, at the layer where it can still be classified.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FetchError {
    /// DNS, TLS, timeout, refused connection.
    #[error("network error: {0}")]
    Network(String),

    /// A non-2xx response, with the endpoint's own summary of why.
    #[error("HTTP {status}: {message}")]
    Http {
        /// The status line's code.
        status: u16,
        /// The endpoint's own summary, compacted to one line.
        message: String,
    },

    /// The body arrived but could not be decoded.
    #[error("could not parse the response: {0}")]
    Parse(String),

    /// The endpoint returned nothing.
    #[error("the query returned no results")]
    Empty,

    /// The body stopped before the result set did.
    ///
    /// Its own variant rather than [`FetchError::Network`] because the two want
    /// opposite behaviour. A network failure is worth repeating; this is a body
    /// that began arriving and then ran out, and repeating it re-downloads
    /// however many hundreds of megabytes already arrived in order to arrive at
    /// the same place. It is also not a parse failure: the CSV read fine, there
    /// is simply less of it than the query asked for, and the only honest
    /// outcome is to refuse the answer rather than report the part that came.
    #[error("the result set was cut short after {bytes_read} bytes: {reason}")]
    Truncated {
        /// How much of the body did arrive, so the message can say the answer
        /// was not empty but was also not complete.
        bytes_read: u64,
        /// Why the reader gave up on it.
        reason: TruncationReason,
    },
}

/// Why a streamed body stopped short.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TruncationReason {
    /// No bytes arrived for longer than the stall watchdog allows, so the body
    /// is alive but going nowhere.
    #[error("no data arrived within the timeout")]
    Stalled,

    /// The connection ended cleanly mid-body, which is what a proxy imposing its
    /// own ceiling looks like from here.
    #[error("the connection closed before the query finished")]
    ClosedEarly,
}

impl From<lotus_query::ParseError> for FetchError {
    /// A payload the pure parser rejected is a bad answer, not a bad request,
    /// so it becomes the same variant a decoder failure would.
    fn from(err: lotus_query::ParseError) -> Self {
        Self::Parse(err.message().to_string())
    }
}

impl FetchError {
    /// Whether a request that failed this way could succeed if sent again.
    ///
    /// A 4xx will not: the query itself is the problem, and repeating it
    /// repeats the rejection. A 429 might, later. A 5xx is the endpoint's.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        match self {
            Self::Network(_) => true,
            Self::Http { status, .. } => is_retryable_status(*status),
            Self::Parse(_) | Self::Empty | Self::Truncated { .. } => false,
        }
    }

    /// Whether this failure is a result set too short to report.
    ///
    /// A caller holding a partial set needs this to refuse it. Without it the
    /// counts derived from the rows that did arrive are internally consistent
    /// and wrong, which is the one outcome worse than an error.
    #[must_use]
    pub const fn is_truncated(&self) -> bool {
        matches!(self, Self::Truncated { .. })
    }

    /// Whether the endpoint looked unreachable, as opposed to rejecting the
    /// query. This is the condition for trying `WDQS` after `QLever`.
    #[must_use]
    pub const fn is_endpoint_unavailable(&self) -> bool {
        matches!(self, Self::Network(_)) || matches!(self, Self::Http { status: 502, .. })
    }

    /// The HTTP status, when the failure was an HTTP one.
    #[must_use]
    pub const fn status(&self) -> Option<u16> {
        match self {
            Self::Http { status, .. } => Some(*status),
            _ => None,
        }
    }
}

/// The response format to ask an endpoint for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseFormat {
    /// `text/csv`, which is what the parsers read.
    Csv,
    /// `application/sparql-results+json`.
    SparqlJson,
    /// `text/turtle`.
    Turtle,
    /// `application/n-triples`.
    NTriples,
}

impl ResponseFormat {
    /// The `Accept` header value.
    #[must_use]
    pub const fn accept(self) -> &'static str {
        match self {
            Self::Csv => "text/csv",
            Self::SparqlJson => "application/sparql-results+json",
            Self::Turtle => "text/turtle",
            Self::NTriples => "application/n-triples",
        }
    }

    /// `QLever`'s `action=` parameter, which is how it selects an export format.
    #[must_use]
    pub const fn qlever_action(self) -> Option<&'static str> {
        match self {
            Self::Csv => Some("csv_export"),
            Self::SparqlJson => Some("sparql_json_export"),
            Self::Turtle => Some("turtle_export"),
            Self::NTriples => None,
        }
    }
}

/// Whether a status code is worth a second attempt.
///
/// A free function so the retry policy is testable without constructing an error,
/// and -- the reason it exists as its own function rather than as an arm inside
/// [`FetchError::is_retryable`] -- so there is **one** rule rather than a copy in
/// each place that retries.
///
/// That it is now the only copy is not a style preference. It was exported and
/// called from nowhere, while `FetchError::is_retryable` carried its own inline
/// `status == 429 || status >= 500`. Mutation testing found it: every mutant of
/// this function survived, because the tests all went through the *copy*. The two
/// could drift, and changing one would have left the retry loop disagreeing with
/// the function that documents it.
#[must_use]
pub const fn is_retryable_status(status: u16) -> bool {
    status == 429 || status >= 500
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rejected_query_is_not_retried_but_a_busy_one_is() {
        let rejected = FetchError::Http {
            status: 400,
            message: "bad".into(),
        };
        assert!(!rejected.is_retryable());
        assert!(
            FetchError::Http {
                status: 429,
                message: "slow down".into()
            }
            .is_retryable()
        );
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
    /// `FetchError::is_retryable`, which is a *separate* implementation of the same
    /// rule -- that duplication is why every mutant of `is_retryable_status`
    /// survived, since nothing reached the function they were mutating. It is now
    /// the only copy, and these tests are what hold it to that.
    #[test]
    fn the_retry_rule_is_429_or_a_server_error() {
        // The client-error side: only 429 is worth another attempt. A 400 is the
        // endpoint saying no, and 404 will not become a 200 on a second try.
        for status in [
            200, 201, 204, 301, 400, 401, 403, 404, 409, 418, 428, 430, 451,
        ] {
            assert!(
                !is_retryable_status(status),
                "{status} is a settled answer and retrying it wastes an attempt"
            );
        }

        // 429 is the one client error that is explicitly "come back later".
        assert!(is_retryable_status(429), "too many requests is retryable");

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
    /// The `Accept` header and the `action=` parameter are the whole of how a
    /// format reaches the endpoint, so a constant in either place is silent: the
    /// request still succeeds and returns the wrong shape, which the parser then
    /// reports as somebody else's bug. `NTriples` has no `action` at all, and that
    /// is a fact about `QLever` rather than an oversight -- pinning it stops a
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
}
