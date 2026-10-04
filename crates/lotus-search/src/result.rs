// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use lotus_model::{CompoundEntry, DatasetStats, Rows, SearchCriteria};

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

/// A whole result set, in memory that fits, with its exact counts.
///
/// What [`search_columnar`](crate::search_columnar) returns. There is no
/// `truncated` field and no separate `stats`: the set holds every row the endpoint
/// returned and computes its own counts, so a caller cannot report a total that
/// disagrees with the rows it is showing.
#[derive(Debug, Clone, Default)]
pub struct ColumnarSearchResult {
    /// Every row, stored by column.
    pub set: lotus_model::ColumnarResultSet,
    /// How the taxon's input was interpreted, or `None` for a structure-only
    /// search.
    pub taxon: Option<TaxonResolution>,
    /// The query that produced the rows, so a caller can download the same set by
    /// another route or show it to the reader.
    pub query: String,
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

#[cfg(test)]
mod tests {
    use super::{SearchResult, TaxonNote, TaxonResolution};
    use lotus_model::CompoundEntry;

    fn resolution(qid: Option<&str>, looked_up: &str) -> TaxonResolution {
        TaxonResolution {
            qid: qid.map(str::to_string),
            looked_up: looked_up.to_string(),
            notes: Vec::new(),
        }
    }

    /// A resolved taxon reports the name the lookup used, and no name for an
    /// empty lookup.
    ///
    /// `looked_up_name` is what the UI shows next to the results, so its two
    /// failure modes are different bugs: reporting a name for a resolution that
    /// has none puts an empty string on screen, and refusing to report a name
    /// that exists leaves the reader with a bare QID and no idea what was searched.
    #[test]
    fn a_resolution_reports_the_name_it_looked_up() {
        assert_eq!(
            resolution(Some("Q16521"), "Gentiana lutea").looked_up_name(),
            Some("Gentiana lutea")
        );
        assert_eq!(
            resolution(None, "Gentianales").looked_up_name(),
            Some("Gentianales"),
            "a wildcard has no QID but still has the text the reader typed"
        );
        assert_eq!(
            resolution(None, "").looked_up_name(),
            None,
            "an empty lookup has no name to report, and must not report an empty one"
        );
        assert_eq!(
            resolution(Some("Q16521"), "").looked_up_name(),
            None,
            "a QID input with no looked-up text reports no name"
        );
    }

    /// Every taxon is the absence of a QID, and nothing else.
    ///
    /// This is what decides whether the result set is described as unfiltered, so
    /// a `*` search and a taxon search must not agree. Both directions matter: the
    /// constant `true` would label every search as covering all taxa, and the
    /// constant `false` would label none of them.
    #[test]
    fn every_taxon_is_exactly_the_absence_of_a_qid() {
        assert!(
            resolution(None, "*").is_all_taxa(),
            "a wildcard covers every taxon"
        );
        assert!(
            resolution(None, "").is_all_taxa(),
            "an empty input constrains no taxon"
        );
        assert!(
            !resolution(Some("Q16521"), "Gentiana lutea").is_all_taxa(),
            "a named taxon is one taxon, not every taxon"
        );
    }

    /// The rows come back as the rows that are stored, in order, and nothing else.
    ///
    /// `as_rows` is the hand-off from a page of rows to a table or an export, so
    /// a default-constructed `Rows` here would be an empty export that reported
    /// success.
    #[test]
    fn the_rows_are_the_rows_that_were_stored() {
        let mut result = SearchResult::default();
        assert!(
            result.as_rows().is_empty(),
            "a result with no rows has no rows"
        );

        result.rows = (1..=3)
            .map(|n| CompoundEntry {
                compound_qid: format!("Q{n}").into(),
                name: format!("compound-{n}").into(),
                ..CompoundEntry::default()
            })
            .collect();

        let rows = result.as_rows();
        assert_eq!(rows.len(), 3, "every stored row is handed over");
        let handed_over: Vec<&str> = rows.iter().map(|r| r.compound_qid.as_ref()).collect();
        assert_eq!(
            handed_over,
            vec!["Q1", "Q2", "Q3"],
            "and in the order they were stored"
        );
    }

    /// Both notes say what they are, in words a reader can act on.
    ///
    /// These strings go straight into the UI as the explanation for a search that
    /// did something other than what was typed, so "the first of N" has to carry
    /// the count and the standardisation has to carry both spellings. A `Display`
    /// that returned nothing would render as an empty note rather than as a
    /// missing one.
    #[test]
    fn a_note_explains_itself() {
        let standardized = TaxonNote::Standardized {
            original: "gentiana lutea".to_string(),
            looked_up: "Gentiana lutea".to_string(),
        }
        .to_string();
        assert!(
            standardized.contains("gentiana lutea") && standardized.contains("Gentiana lutea"),
            "a standardisation has to name both what was typed and what was used: \
             {standardized}"
        );

        let ambiguous = TaxonNote::Ambiguous {
            candidates: vec![
                "Gentiana lutea (Q16521)".to_string(),
                "Gentiana (Q21754)".to_string(),
            ],
        }
        .to_string();
        assert!(
            ambiguous.contains('2'),
            "an ambiguity has to say how many matched: {ambiguous}"
        );
        assert!(
            ambiguous.contains("Gentiana lutea (Q16521)"),
            "and name them: {ambiguous}"
        );
    }
}
