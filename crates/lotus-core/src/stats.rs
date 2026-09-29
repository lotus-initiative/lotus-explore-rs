// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::CompoundEntry;
use std::collections::HashSet;

/// Counts describing a result set, computed either by the endpoint's `COUNT`
/// query or locally from the rows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DatasetStats {
    /// Distinct compounds.
    pub n_compounds: usize,
    /// Distinct taxa. Compounds with no occurrence do not contribute.
    pub n_taxa: usize,
    /// Distinct references.
    pub n_references: usize,
    /// Result rows.
    pub n_entries: usize,
    /// Distinct compound-taxon-reference triples.
    pub n_entries_unique: usize,
}

impl DatasetStats {
    /// Count from rows that have already been deduplicated.
    ///
    /// `n_entries` therefore equals `n_entries_unique`, and cannot be the raw
    /// row count: the dedup happens during parsing, before the rows arrive
    /// here. Callers that need the raw count use the endpoint's `COUNT` query,
    /// or a reader that counts before deduplicating.
    #[must_use]
    pub fn from_deduplicated_entries(entries: &[CompoundEntry]) -> Self {
        let mut compounds = HashSet::with_capacity(entries.len());
        let mut taxa = HashSet::with_capacity(entries.len());
        let mut references = HashSet::with_capacity(entries.len());
        let mut triples = HashSet::with_capacity(entries.len());

        for e in entries {
            compounds.insert(&*e.compound_qid);
            if !e.taxon_qid.is_empty() {
                taxa.insert(&*e.taxon_qid);
            }
            if !e.reference_qid.is_empty() {
                references.insert(&*e.reference_qid);
            }
            triples.insert((&*e.compound_qid, &*e.taxon_qid, &*e.reference_qid));
        }

        Self {
            n_compounds: compounds.len(),
            n_taxa: taxa.len(),
            n_references: references.len(),
            n_entries: entries.len(),
            n_entries_unique: triples.len(),
        }
    }
}

/// How a structure string is matched against the endpoint's index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SmilesSearchType {
    /// The query structure is a substructure of the indexed compound.
    #[default]
    Substructure,
    /// The query structure is at least `structure_threshold` similar.
    Similarity,
}

impl SmilesSearchType {
    /// The spelling used in URLs, JSON and on the command line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Substructure => "substructure",
            Self::Similarity => "similarity",
        }
    }

    /// Parse the spelling above, or anything else as substructure.
    ///
    /// Infallible by design: an unrecognised value narrows to the default
    /// rather than failing a search over a bad URL parameter.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        if s.trim().eq_ignore_ascii_case("similarity") {
            Self::Similarity
        } else {
            Self::Substructure
        }
    }
}

impl std::fmt::Display for SmilesSearchType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether an optional element may, must, or must not appear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ElementState {
    /// Unconstrained.
    #[default]
    Allowed,
    /// Must be present.
    Required,
    /// Must be absent.
    Excluded,
}

impl ElementState {
    /// The spelling used in URLs, JSON and on the command line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::Required => "required",
            Self::Excluded => "excluded",
        }
    }

    /// Infallible, like [`SmilesSearchType::parse`].
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "required" => Self::Required,
            "excluded" => Self::Excluded,
            _ => Self::Allowed,
        }
    }
}

impl std::fmt::Display for ElementState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn entry(compound: &str, taxon: &str, reference: &str) -> CompoundEntry {
        CompoundEntry {
            compound_qid: Arc::from(compound),
            taxon_qid: Arc::from(taxon),
            reference_qid: Arc::from(reference),
            ..CompoundEntry::default()
        }
    }

    #[test]
    fn distinct_ids_are_counted_not_rows() {
        let rows = vec![
            entry("Q1", "Q10", "Q100"),
            entry("Q1", "Q11", "Q100"),
            entry("Q2", "Q10", "Q101"),
        ];
        let s = DatasetStats::from_deduplicated_entries(&rows);
        assert_eq!(s.n_entries, 3);
        assert_eq!(s.n_entries_unique, 3);
        assert_eq!(s.n_compounds, 2);
        assert_eq!(s.n_taxa, 2);
        assert_eq!(s.n_references, 2);
    }

    #[test]
    fn an_absent_taxon_or_reference_does_not_become_an_id() {
        // Otherwise every un-referenced compound would share one phantom taxon.
        let rows = vec![entry("Q1", "", ""), entry("Q2", "", "")];
        let s = DatasetStats::from_deduplicated_entries(&rows);
        assert_eq!(s.n_compounds, 2);
        assert_eq!(s.n_taxa, 0);
        assert_eq!(s.n_references, 0);
    }

    #[test]
    fn already_deduplicated_input_reports_one_count_not_two() {
        let rows = vec![entry("Q1", "Q10", "Q100")];
        let s = DatasetStats::from_deduplicated_entries(&rows);
        assert_eq!(s.n_entries, s.n_entries_unique);
    }

    #[test]
    fn search_type_and_element_state_round_trip_through_their_spellings() {
        for v in [SmilesSearchType::Substructure, SmilesSearchType::Similarity] {
            assert_eq!(SmilesSearchType::parse(v.as_str()), v);
        }
        for v in [
            ElementState::Allowed,
            ElementState::Required,
            ElementState::Excluded,
        ] {
            assert_eq!(ElementState::parse(v.as_str()), v);
        }
    }

    #[test]
    fn an_unrecognised_spelling_falls_back_to_the_default() {
        assert_eq!(
            SmilesSearchType::parse("fuzzy"),
            SmilesSearchType::Substructure
        );
        assert_eq!(ElementState::parse("maybe"), ElementState::Allowed);
    }
}
