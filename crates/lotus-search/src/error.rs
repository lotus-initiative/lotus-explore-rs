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
            Self::Http { status, .. } => *status == 429 || *status >= 500,
            Self::Parse(_) | Self::Empty => false,
        }
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
/// Free function so that the retry policy is testable without constructing an
/// error, and so the two paths that retry agree on the rule.
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
}
