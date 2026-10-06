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
    /// waiting, retrying or reconnecting cannot help — the query will not become
    /// cheaper. Only a narrower query helps, so the message says that instead of
    /// offering a button whose second click spends the budget again.
    QueryTooExpensive,
    Parse,
    /// The result set stopped short of the whole answer.
    ///
    /// Its own kind because the generic hint is wrong here. Every other failure
    /// suggests something changeable — retry, fix the query, check the network —
    /// and this one must not: the rows on screen are a fraction of the real set,
    /// so the message says the answer is incomplete.
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

/// Something true about how a search's free-text input resolved, formatted by
/// the UI layer.
///
/// Named for the act because both things resolve: the taxon field becomes a QID,
/// the structure field a Wikidata compound. One channel so every fact about a
/// search reaches the notice bar together instead of one being dropped for room.
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
    /// Wikidata for a lookup already done. But a common name is what prose calls
    /// the organism rather than what the compounds are filed under, and the same
    /// word names unrelated taxa across languages, so the reader is told what it
    /// resolved to.
    CommonName {
        chosen_name: String,
        chosen_qid: String,
    },
    /// A structure field resolved to a Wikidata compound, by one of the routes
    /// that is not an identifier.
    ///
    /// Separate from [`LookupNotice::CommonName`]: a taxon common name is a poor
    /// way to pick an organism, while a compound's label or alias is how that
    /// compound is written about, and only the structure mode changes what
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
    /// Split from [`LookupNotice::AmbiguousTaxon`] for the same reason
    /// [`LookupNotice::CompoundResolved`] is split from
    /// [`LookupNotice::CommonName`]: same next move, different sentence.
    /// "Ambiguous taxon name" on a structure search names the wrong kind of thing
    /// and points at a field never typed into.
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
}

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
    /// A transport error for `stage`, carrying the repository error it came from.
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
#[path = "types/tests.rs"]
mod tests;
