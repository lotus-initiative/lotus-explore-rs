// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use lotus_core::{CompoundEntry, DatasetStats, Rows, SearchCriteria};

/// A resolved search: the rows, what the endpoint said about the whole result
/// set, and how the taxon was resolved.
#[derive(Debug, Clone, Default)]
pub struct SearchResult {
    /// The rows, already deduplicated and capped at the requested limit.
    pub rows: Vec<CompoundEntry>,
    /// Counts for the whole result set, not just the returned page.
    pub stats: Option<DatasetStats>,
    /// How the taxon's input was interpreted, or `None` for a structure-only
    /// search.
    pub taxon: Option<TaxonResolution>,
    /// The query that produced the rows, after filters and before the limit, so
    /// that a caller can download the whole result rather than one page.
    pub query: String,
    /// Whether the row limit hid part of the result.
    pub truncated: bool,
}

impl SearchResult {
    /// The rows as a shared slice, which is what a table or an export wants.
    #[must_use]
    pub fn as_rows(&self) -> Rows {
        self.rows.as_slice().into()
    }
}

/// How a taxon's input was interpreted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxonResolution {
    /// The QID to filter on, or `None` for every taxon.
    pub qid: Option<String>,
    /// The text that was looked up, which is the taxon's display name when the
    /// input was a name, and its QID when the input was one.
    pub looked_up: String,
    /// Things worth telling the user about how it was interpreted. Both
    /// together are possible: an input that was both rewritten and ambiguous.
    pub notes: Vec<TaxonNote>,
}

impl TaxonResolution {
    /// Whether the search covers every taxon.
    #[must_use]
    pub const fn is_all_taxa(&self) -> bool {
        self.qid.is_none()
    }

    /// The taxon's display name, when the lookup reported one.
    #[must_use]
    pub fn looked_up_name(&self) -> Option<&str> {
        (!self.looked_up.is_empty()).then_some(self.looked_up.as_str())
    }
}

/// Something the user should know about how their taxon was resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaxonNote {
    /// The input was rewritten before lookup, so the query used different text.
    Standardized {
        /// What the user typed.
        original: String,
        /// What was looked up instead.
        looked_up: String,
    },
    /// More than one taxon matched, and the first was used.
    Ambiguous {
        /// `"Name (QID)"` for each candidate considered.
        candidates: Vec<String>,
    },
}

impl std::fmt::Display for TaxonNote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Standardized {
                original,
                looked_up,
            } => {
                write!(f, "taxon {original:?} was read as {looked_up:?}")
            }
            Self::Ambiguous { candidates } => {
                write!(
                    f,
                    "several taxa matched; using the first of {}",
                    candidates.join(", ")
                )
            }
        }
    }
}

/// The input to a search, before resolution.
#[derive(Debug, Clone)]
pub struct SearchRequest {
    /// The filters to apply.
    pub criteria: SearchCriteria,
    /// The year the year filter is bounded against.
    pub year_max: u16,
    /// Rows to fetch. `None` means [`DEFAULT_ROW_LIMIT`](crate::DEFAULT_ROW_LIMIT).
    pub limit: Option<usize>,
}

impl SearchRequest {
    /// A request for every row the criteria matches.
    #[must_use]
    pub const fn new(criteria: SearchCriteria, year_max: u16) -> Self {
        Self {
            criteria,
            year_max,
            limit: None,
        }
    }

    /// Cap the number of rows returned. The counts still describe the whole
    /// result set.
    #[must_use]
    pub const fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}
