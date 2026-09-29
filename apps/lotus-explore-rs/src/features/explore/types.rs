// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Core domain types for the Explore feature.

use thiserror::Error;

use crate::features::explore::transport_classification::{
    TransportFailureKind, classify_transport_error,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryPhase {
    Idle,
    PreparingQuery,
    ResolvingTaxon,
    FetchingResults,
    ProcessingResults,
    Rendering,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ErrorKind {
    Validation,
    Configuration,
    /// HTTP 4xx — the request itself was invalid; retrying the same request will not help.
    BadRequest,
    /// Network/transport failure — may be transient, retry may succeed.
    Network,
    /// Upstream service rate-limited this request.
    RateLimit,
    Parse,
    #[cfg(target_arch = "wasm32")]
    Memory,
    #[default]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryStage {
    TaxonSearch,
    ResultsQuery,
}

impl QueryStage {
    pub const fn as_key(self) -> &'static str {
        match self {
            Self::TaxonSearch => "taxon_search",
            Self::ResultsQuery => "results_query",
        }
    }
}

impl std::fmt::Display for QueryStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_key())
    }
}

// ── Taxon warning (structured, formatted at UI boundary) ─────────────────────

/// A structured warning about taxon resolution, formatted by the UI layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaxonWarning {
    /// The raw input was normalized before lookup.
    Standardized {
        original: String,
        standardized: String,
    },
    /// Multiple candidates found; `chosen_*` is the one we used.
    Ambiguous {
        chosen_name: String,
        chosen_qid: String,
        /// Top candidates as `"Name (QID)"` strings.
        candidates: Vec<String>,
    },
    /// Raw warning string received from the REST API response.
    ApiMessage(String),
    /// Query executed against Wikidata Query Service after a `QLever` fallback.
    WdqsFallback,
}

// ── Domain error hierarchy (i18n-free) ───────────────────────────────────────

/// Fine-grained validation fault.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ValidationFault {
    #[error("empty input")]
    EmptyInput,
    #[error("taxon is too long")]
    TaxonTooLong,
    #[error("structure input is too long")]
    StructureTooLong,
    #[error("mass is out of range")]
    MassOutOfRange,
    #[error("mass range is invalid")]
    MassRangeInvalid,
    #[error("year is out of range")]
    YearOutOfRange,
    #[error("year range is invalid")]
    YearRangeInvalid,
    #[error("element count is too high")]
    ElementCountTooHigh,
    #[error("similarity threshold must be greater than 0")]
    SimilarityThresholdInvalid,
    #[error("taxon not found: {input}")]
    TaxonNotFound { input: String },
    #[error("unsupported download format: {format}")]
    UnsupportedFormat { format: String },
}

/// Fine-grained CSV / data parse fault.
/// Parse variants are scoped to active Explore pipeline stages.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ParseFault {
    #[error("taxon csv parse failed: {details}")]
    TaxonCsv { details: String },
    #[error("taxon candidate selection failed: {details}")]
    TaxonPick { details: String },
    #[error("results csv parse failed: {details}")]
    ResultsCsv { details: String },
}

/// Top-level domain error used throughout the Explore feature.
/// Contains **no locale-dependent strings**; UI components format errors at render time.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum DomainError {
    #[error("validation: {0}")]
    Validation(ValidationFault),

    #[error("transport at {stage}: {source}")]
    Transport {
        stage: QueryStage,
        #[source]
        source: crate::repositories::RepositoryError,
    },

    #[error("parse: {0}")]
    Parse(ParseFault),

    #[cfg(target_arch = "wasm32")]
    #[error("memory limit reached during {stage}")]
    MemoryLimit { stage: QueryStage },
}

impl DomainError {
    /// Construct a transport error for the given query stage and repository source.
    /// This function is primarily used in unit tests and benchmarks to construct
    /// error scenarios for testing error handling and propagation logic.
    #[cfg(test)]
    pub fn transport(stage: QueryStage, source: crate::repositories::RepositoryError) -> Self {
        Self::Transport { stage, source }
    }

    /// Map a repository error to a transport domain error via `.map_err`.
    pub fn transport_at(
        stage: QueryStage,
    ) -> impl Fn(crate::repositories::RepositoryError) -> Self {
        move |source| Self::Transport { stage, source }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn memory_limit(stage: QueryStage) -> Self {
        Self::MemoryLimit { stage }
    }

    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Validation(_) => ErrorKind::Validation,
            Self::Transport { source, .. } => match classify_transport_error(source) {
                TransportFailureKind::Configuration => ErrorKind::Configuration,
                TransportFailureKind::BadRequest | TransportFailureKind::QuerySyntax => {
                    ErrorKind::BadRequest
                }
                TransportFailureKind::Parse => ErrorKind::Parse,
                TransportFailureKind::RateLimit => ErrorKind::RateLimit,
                TransportFailureKind::Network
                | TransportFailureKind::Server
                | TransportFailureKind::CacheConflict => ErrorKind::Network,
            },
            Self::Parse(_) => ErrorKind::Parse,
            #[cfg(target_arch = "wasm32")]
            Self::MemoryLimit { .. } => ErrorKind::Memory,
        }
    }

    /// Extract the query stage at which this error occurred, if applicable.
    pub const fn query_stage(&self) -> QueryStage {
        match self {
            Self::Transport { stage, .. } => *stage,
            #[cfg(target_arch = "wasm32")]
            Self::MemoryLimit { stage } => *stage,
            // For validation and parse errors, assume results stage as fallback
            _ => QueryStage::ResultsQuery,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]

    use super::*;

    #[test]
    fn transport_domain_error_exposes_repository_source() {
        let err = DomainError::transport(
            QueryStage::ResultsQuery,
            crate::repositories::RepositoryError::network("timeout"),
        );

        match err {
            DomainError::Transport { source, .. } => {
                assert_eq!(source.to_string(), "network error: timeout");
            }
            _ => panic!("expected transport error"),
        }
    }

    #[test]
    fn transport_http_4xx_is_classified_as_bad_request() {
        let err = DomainError::transport(
            QueryStage::ResultsQuery,
            crate::repositories::RepositoryError::Http {
                status: 400,
                body: "invalid query".into(),
            },
        );

        assert_eq!(err.kind(), ErrorKind::BadRequest);
    }

    #[test]
    fn transport_network_is_classified_as_network() {
        let err = DomainError::transport(
            QueryStage::ResultsQuery,
            crate::repositories::RepositoryError::network("timeout"),
        );

        assert_eq!(err.kind(), ErrorKind::Network);
    }

    #[test]
    fn transport_parse_is_classified_as_parse() {
        let err = DomainError::transport(
            QueryStage::ResultsQuery,
            crate::repositories::RepositoryError::parse("csv decode failed"),
        );

        assert_eq!(err.kind(), ErrorKind::Parse);
    }

    #[test]
    fn transport_not_configured_is_classified_as_configuration() {
        let err = DomainError::transport(
            QueryStage::ResultsQuery,
            crate::repositories::RepositoryError::NotConfigured,
        );

        assert_eq!(err.kind(), ErrorKind::Configuration);
    }

    #[test]
    fn transport_http_syntax_error_is_classified_as_bad_request() {
        let err = DomainError::transport(
            QueryStage::ResultsQuery,
            crate::repositories::RepositoryError::Http {
                status: 400,
                body: "Invalid SPARQL query: mismatched input 'AS' expecting ','".into(),
            },
        );

        assert_eq!(err.kind(), ErrorKind::BadRequest);
    }

    #[test]
    fn transport_at_closure_maps_repository_error() {
        let mapper = DomainError::transport_at(QueryStage::ResultsQuery);
        let err = mapper(crate::repositories::RepositoryError::network("timed out"));
        assert!(matches!(
            err,
            DomainError::Transport {
                stage: QueryStage::ResultsQuery,
                ..
            }
        ));
    }
}
