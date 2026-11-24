// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Canonical SPARQL endpoint URLs and metadata descriptors shared across all
//! LOTUS crates and front ends.

/// `QLever`'s Wikidata endpoint. Faster than WDQS, and the default.
pub const QLEVER_WIKIDATA: &str = "https://qlever.dev/api/wikidata";

/// The Wikidata Query Service (v2, `QLever`-backed), used when `QLever` is unreachable.
pub const WDQS_WIKIDATA: &str = "https://query-next.wikidata.org/sparql";

/// WDQS v2's scholarly subgraph, which carries the reference properties P1476, P356
/// and P577 that the main endpoint does not host locally.
pub const WDQS_SCHOLARLY: &str = "https://query-scholarly-next.wikidata.org/sparql";

/// Represents the SPARQL endpoint used for a query.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SparqlEndpoint {
    /// `QLever` endpoint (default/preferred).
    #[default]
    Qlever,
    /// Wikidata Query Service (fallback when `QLever` is unavailable).
    Wdqs,
}

impl std::fmt::Display for SparqlEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Qlever => write!(f, "QLever"),
            Self::Wdqs => write!(f, "Wikidata Query Service"),
        }
    }
}

impl SparqlEndpoint {
    /// The endpoint's URL, name, and description, as the metadata document
    /// reports them.
    #[must_use]
    pub const fn info(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Qlever => (
                QLEVER_WIKIDATA,
                "QLever Wikidata",
                "Fast SPARQL endpoint for Wikidata (QLever)",
            ),
            Self::Wdqs => (
                WDQS_WIKIDATA,
                "Wikidata Query Service",
                "Wikidata Query Service (QLever fallback)",
            ),
        }
    }
}
