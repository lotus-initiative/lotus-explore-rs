// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use thiserror::Error;

/// A failed request, at the layer where it can still be classified.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FetchError {
    /// DNS, TLS, timeout, refused connection.
    #[error("network error: {0}")]
    Network(String),

    /// The endpoint cancelled the query because it ran out of time.
    ///
    /// `QLever` answers **`429` when a query exceeds its time limit**, not when a client
    /// sends too many requests. Verified against `qlever.dev` on 2026-10-04:
    ///
    /// ```text
    /// POST /api/wikidata  timeout=3s   ->  429
    ///   {"exception": "Operation timed out. Last operation: Sort (internal order) on ?r"}
    /// POST /api/wikidata  timeout=60s  ->  403
    ///   {"exception": "User submitted timeout was higher than what is currently
    ///                  allowed by this instance (30s)."}
    /// ```
    ///
    /// A 429 says **one query is too expensive**, and retrying is the one thing that cannot
    /// help: the same query runs the same 30 seconds and is cancelled for the same reason,
    /// while occupying the endpoint for the full budget each time -- how a client becomes
    /// the kind an operator blocks.
    ///
    /// Hence **not** [`Self::is_retryable`], and deliberately not
    /// [`Self::is_endpoint_unavailable`] either: falling back to `WDQS` would not make an
    /// expensive query cheap, it would run it twice on two endpoints.
    #[error("the query exceeded the endpoint's time limit ({}): {message}", budget.as_deref().unwrap_or("unset"))]
    TimedOut {
        /// The budget that was asked for, in the endpoint's own duration syntax.
        budget: Option<String>,
        /// The endpoint's own account of where it stopped.
        message: String,
    },

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
    /// Its own variant rather than [`FetchError::Network`] because the two want opposite
    /// behaviour. A network failure is worth repeating; this is a body that began arriving and
    /// ran out, and repeating it re-downloads however many hundreds of megabytes already
    /// arrived to arrive in the same place. Not a parse failure either: the CSV read fine,
    /// there is simply less of it than the query asked for, so the honest outcome is to refuse
    /// the answer rather than report the part that came.
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
    /// A 4xx will not: the query itself is the problem, and repeating it repeats the rejection.
    /// A 5xx is the endpoint's.
    ///
    /// `TimedOut` is not retryable because `QLever`'s 429 is a query time limit: a second
    /// attempt spends another full budget to arrive at the same cancellation.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        match self {
            Self::Network(_) => true,
            Self::Http { status, .. } => is_retryable_status(*status),
            Self::TimedOut { .. } | Self::Parse(_) | Self::Empty | Self::Truncated { .. } => false,
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

    /// Whether this query outstayed the endpoint's time budget.
    ///
    /// Distinct from "not retryable": this is the one failure where the caller
    /// can still do something useful, by asking for less. It is what the UI
    /// turns into "this search is too broad", rather than "try again".
    #[must_use]
    pub const fn is_timed_out(&self) -> bool {
        matches!(self, Self::TimedOut { .. })
    }

    /// The HTTP status, when the failure was an HTTP one.
    #[must_use]
    pub const fn status(&self) -> Option<u16> {
        match self {
            Self::Http { status, .. } => Some(*status),
            // A cancellation is a 429 on the wire, and the classification above
            // this crate reads statuses. Reporting it as one keeps every caller
            // that has never heard of `TimedOut` behaving as it did.
            Self::TimedOut { .. } => Some(429),
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
/// A free function so the retry policy is testable without constructing an error, and so
/// there is **one** rule rather than a copy in each place that retries.
///
/// That it is now the only copy is not a style preference. It was exported and called from
/// nowhere while `FetchError::is_retryable` carried its own inline
/// `status == 429 || status >= 500`. Mutation testing found it: every mutant of this
/// function survived, because the tests all went through the *copy*. The two could drift,
/// and changing one would leave the retry loop disagreeing with the function documenting it.
///
/// **`429` is excluded deliberately.** `QLever` answers 429 when the query itself ran out of
/// time (`src/engine/Server.cpp`, `CancellationException` -> 429, "Operation timed out"), not
/// "too many requests". Retrying one spends the endpoint's entire budget again to be
/// cancelled the same way, so a single broad search could cost four full-length queries.
/// [`FetchError::TimedOut`] carries the measurement.
#[must_use]
pub const fn is_retryable_status(status: u16) -> bool {
    status >= 500
}

#[cfg(test)]
#[path = "error/tests.rs"]
mod tests;
