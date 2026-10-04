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
    /// A name or `InChIKey` in the structure field is being looked up.
    ResolvingStructure,
    /// A QID or DOI in the reference field is being looked up.
    ResolvingReference,
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
    /// The endpoint cancelled the query because it outran its time budget.
    ///
    /// Its own kind because the advice is the opposite of every other failure's:
    /// nothing about waiting, retrying or reconnecting helps, because the query
    /// is not going to become cheaper. The only thing that helps is a narrower
    /// one, so the message says that instead of offering a button whose second
    /// click spends the endpoint's whole budget again.
    QueryTooExpensive,
    Parse,
    /// The result set stopped short of the whole answer.
    ///
    /// Its own kind because the generic hint is wrong here. Every other failure
    /// suggests something the user can change — retry, fix the query, check the
    /// network — and this one must not: the rows on screen would be a fraction
    /// of the real set, so the message has to say the answer is incomplete
    /// rather than suggest trying again and implying that will fix it.
    Truncated,
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

// ── Lookup notices (structured, formatted at UI boundary) ────────────────────

/// Something true about how a search's free-text input resolved, formatted by
/// the UI layer.
///
/// Named for the act rather than for one of the two things that resolves,
/// because both do: the taxon field becomes a QID and the structure field
/// becomes a Wikidata compound. They share a channel so that every fact about a
/// search reaches the notice bar together, rather than one of them having to be
/// dropped because there was only room for one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LookupNotice {
    /// The raw input was normalized before lookup.
    Standardized {
        original: String,
        standardized: String,
    },
    /// The name matched a taxon's common name (`P1843`) rather than its
    /// scientific name (`P225`).
    ///
    /// It resolved, which is the point: refusing it would send the reader to
    /// Wikidata to perform a lookup this tool has already done. But a common
    /// name is what prose calls the organism rather than what the compounds are
    /// filed under, and the same word is used for unrelated taxa in different
    /// languages, so the reader is told what it resolved to.
    CommonName {
        chosen_name: String,
        chosen_qid: String,
    },
    /// A structure field resolved to a Wikidata compound, by one of the routes
    /// that is not an identifier.
    ///
    /// Named rather than reused from [`LookupNotice::CommonName`] because the
    /// two are not the same advice: a taxon common name is a poor way to pick an
    /// organism, while a compound's label or alias is how that compound is
    /// actually written about, and only the structure search mode changes what
    /// happens next.
    CompoundResolved {
        /// The label or alias that was typed.
        chosen_label: String,
        /// The compound it resolved to.
        chosen_qid: String,
    },
    /// More than one taxon matched; `chosen_*` is the one that was used.
    AmbiguousTaxon {
        chosen_name: String,
        chosen_qid: String,
        /// Top candidates as `"Name (QID)"` strings.
        candidates: Vec<String>,
    },
    /// More than one compound matched a structure or a label; `chosen_*` is the
    /// one that was used.
    ///
    /// Split from [`LookupNotice::AmbiguousTaxon`] rather than shared with it,
    /// for the same reason [`LookupNotice::CompoundResolved`] is split from
    /// [`LookupNotice::CommonName`]: the reader's next move is the same, but the
    /// sentence is not. "Ambiguous taxon name" on a structure search names the
    /// wrong kind of thing, and points the reader at the field they did not type
    /// into.
    AmbiguousCompound {
        chosen_name: String,
        chosen_qid: String,
        /// Top candidates as `"Name (QID)"` strings.
        candidates: Vec<String>,
    },
    /// Raw warning string received from the REST API response.
    ApiMessage(String),
    /// Query executed against Wikidata Query Service after a `QLever` fallback.
    WdqsFallback,
    /// The search names neither a structure nor a taxon, so it covers every
    /// compound LOTUS holds rather than a subset of one.
    ///
    /// Carried as a notice rather than refused, because it is a legitimate
    /// question -- "everything published in 2019" is a real one, and a mass or year
    /// filter on its own is a perfectly good way to ask it. It is worth saying out
    /// loud, though, because the answer is the whole database and the table can
    /// only show the first page of it.
    Unconstrained,
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
    #[error("compound not found: {input}")]
    CompoundNotFound { input: String },
    #[error("taxon not found: {input}")]
    TaxonNotFound { input: String },
    #[error("reference not found: {input}")]
    ReferenceNotFound { input: String },
    #[error("a reference must be a Wikidata QID or a DOI: {input}")]
    ReferenceNotAnIdentifier { input: String },
    #[error("unsupported download format: {format}")]
    UnsupportedFormat { format: String },
}

/// Fine-grained CSV / data parse fault.
/// Parse variants are scoped to active Explore pipeline stages.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ParseFault {
    #[error("taxon csv parse failed: {details}")]
    TaxonCsv { details: String },
    #[error("compound csv parse failed: {details}")]
    CompoundCsv { details: String },
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
                TransportFailureKind::Truncated => ErrorKind::Truncated,
                TransportFailureKind::RateLimit => ErrorKind::RateLimit,
                TransportFailureKind::QueryTooExpensive => ErrorKind::QueryTooExpensive,
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
