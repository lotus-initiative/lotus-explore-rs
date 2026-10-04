// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::CompoundEntry;
use std::collections::HashSet;

/// `Display` for an enum whose printed form is its [`as_str`](Self::as_str).
///
/// Every enum in this module that has an `as_str` wants exactly this, and the two
/// that had one were written out by hand -- which `cargo dejadoc` reports, and
/// rightly: the second copy is where the next edit lands without the first being
/// looked at.
///
/// A macro rather than a generic because `Display` is implemented for a concrete
/// type, and a helper rather than a macro because the body is a single call whose
/// only content is *which* method to delegate to. It has to be a macro: there is no
/// way to write one function that implements `Display` for somebody else's type.
///
/// Invoked once, below [`ElementState`], because an `impl` block cannot precede
/// the type it names. That is the cost of collecting them, and it is paid once
/// rather than once per enum.
macro_rules! display_via_as_str {
    ($($type:ty),+ $(,)?) => {
        $(
            impl std::fmt::Display for $type {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str(self.as_str())
                }
            }
        )+
    };
}

/// Counts describing a result set, computed either by the endpoint's `COUNT`
/// query or locally from the rows.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
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

/// How a structure literal is matched against the endpoint's index.
///
/// Whatever the structure field holds is resolved to a Wikidata compound first --
/// a QID, an `InChIKey` or a name by a lookup query, a SMILES or a molfile by
/// asking the structure service which compound it is. This type then decides what
/// happens to that compound.
///
/// [`Self::Exact`] is the default because "this compound" is what a reader who
/// types a compound is asking for, and it is the cheap answer: one row, found by
/// an index scan on the QID. The other two are broader questions about *other*
/// compounds, and both of them load a structure index and score every candidate in
/// it. They have to be asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub enum SmilesSearchType {
    /// Only the compound the input resolved to. No structure service.
    #[default]
    Exact,
    /// The query structure is a substructure of the indexed compound.
    Substructure,
    /// The query structure is at least `structure_threshold` similar.
    Similarity,
}

impl SmilesSearchType {
    /// Whether answering this one requires the structure service.
    ///
    /// Exact does not, and the query is built accordingly. This is the single
    /// place that says so, so the builder and the panel cannot pick differently.
    #[must_use]
    pub const fn needs_structure_service(self) -> bool {
        !matches!(self, Self::Exact)
    }
}

/// The Tanimoto cutoff a similarity search uses when the reader does not set one.
///
/// 1.0 means an identical fingerprint: the same molecule. A default below that
/// would return near-neighbours to a reader who did not ask for any, which is the
/// same failure as answering a name with a substructure search -- more rows, and
/// none of them the compound that was named.
pub const DEFAULT_STRUCTURE_THRESHOLD: f64 = 1.0;

impl SmilesSearchType {
    /// The spelling used in URLs, JSON and on the command line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Substructure => "substructure",
            Self::Similarity => "similarity",
        }
    }

    /// Parse the spelling above, or anything else as the default.
    ///
    /// Infallible by design: an unrecognised value narrows to the default
    /// rather than failing a search over a bad URL parameter.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        if s.trim().eq_ignore_ascii_case("substructure") {
            Self::Substructure
        } else if s.trim().eq_ignore_ascii_case("similarity") {
            Self::Similarity
        } else {
            Self::Exact
        }
    }
}

/// Whether an optional element may, must, or must not appear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
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

display_via_as_str!(SmilesSearchType, ElementState);
impl DatasetStats {
    /// The counts implied by a set of rows, without asking the endpoint.
    ///
    /// This is the same arithmetic as the `COUNT(DISTINCT …)` in the count
    /// query, and it exists for the case where that query failed or was not
    /// asked for: a page of results still has to say how big the result set is,
    /// and answering "500 rows" when 12,000 matched is worse than saying
    /// "at least 500".
    ///
    /// A row with no taxon or no reference still counts towards its compound,
    /// but contributes no taxon and no reference: a compound with no
    /// occurrence is not a taxon.
    #[must_use]
    pub fn from_entries(entries: &[CompoundEntry]) -> Self {
        let mut compounds: HashSet<&str> = HashSet::with_capacity(entries.len());
        let mut taxa: HashSet<&str> = HashSet::with_capacity(entries.len());
        let mut references: HashSet<&str> = HashSet::with_capacity(entries.len());
        let mut triples: HashSet<(&str, &str, &str)> = HashSet::with_capacity(entries.len());

        for entry in entries {
            compounds.insert(entry.compound_qid.as_ref());
            if !entry.taxon_qid.is_empty() {
                taxa.insert(entry.taxon_qid.as_ref());
            }
            if !entry.reference_qid.is_empty() {
                references.insert(entry.reference_qid.as_ref());
            }
            triples.insert((
                entry.compound_qid.as_ref(),
                entry.taxon_qid.as_ref(),
                entry.reference_qid.as_ref(),
            ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::criteria::SearchCriteria;

    #[test]
    fn a_default_search_asks_for_that_one_compound() {
        assert_eq!(SmilesSearchType::default(), SmilesSearchType::Exact);
        assert!(
            !SmilesSearchType::Exact.needs_structure_service(),
            "the default must not load a structure index to answer a name"
        );
        assert!(SmilesSearchType::Substructure.needs_structure_service());
        assert!(SmilesSearchType::Similarity.needs_structure_service());

        let criteria = SearchCriteria::up_to_year(2026);
        assert_eq!(criteria.structure_search, SmilesSearchType::Exact);
        assert_eq!(criteria.structure_threshold, 1.0);
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
        assert_eq!(SmilesSearchType::parse("fuzzy"), SmilesSearchType::Exact);
        assert_eq!(ElementState::parse("maybe"), ElementState::Allowed);
    }
}

impl std::str::FromStr for ElementState {
    type Err = std::convert::Infallible;

    /// Unknown text reads as [`ElementState::Allowed`], not as an error.
    ///
    /// This is a form control's value coming back from a URL, and a URL is
    /// something a person can edit. Rejecting the whole search because
    /// `f_state=maybe` is not one of three values would be a worse answer than
    /// ignoring the part that is nonsense.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::parse(s))
    }
}

#[cfg(test)]
mod spelling_tests {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::{ElementState, SmilesSearchType};

    // The spellings are the wire format: a form control's value comes back in a
    // URL, and the URL is something a person can edit. Each variant therefore
    // has exactly one string, and a change to it is a change to every saved
    // search.

    #[test]
    fn each_search_type_prints_exactly_its_own_spelling() {
        assert_eq!(SmilesSearchType::Substructure.to_string(), "substructure");
        assert_eq!(SmilesSearchType::Similarity.to_string(), "similarity");
    }

    #[test]
    fn each_element_state_prints_exactly_its_own_spelling() {
        assert_eq!(ElementState::Allowed.to_string(), "allowed");
        assert_eq!(ElementState::Required.to_string(), "required");
        assert_eq!(ElementState::Excluded.to_string(), "excluded");
    }

    #[test]
    fn parsing_is_case_and_whitespace_insensitive() {
        for (text, expected) in [
            ("substructure", SmilesSearchType::Substructure),
            ("  SUBSTRUCTURE  ", SmilesSearchType::Substructure),
            ("Similarity", SmilesSearchType::Similarity),
        ] {
            assert_eq!(SmilesSearchType::parse(text), expected, "{text:?}");
        }
        for (text, expected) in [
            ("allowed", ElementState::Allowed),
            (" REQUIRED ", ElementState::Required),
            ("Excluded", ElementState::Excluded),
        ] {
            assert_eq!(
                text.parse::<ElementState>().unwrap_or_default(),
                expected,
                "{text:?}"
            );
        }
    }

    #[test]
    fn printing_then_parsing_returns_the_same_value() {
        // A round trip is what keeps a shared link working after a rename.
        for search in [SmilesSearchType::Substructure, SmilesSearchType::Similarity] {
            assert_eq!(SmilesSearchType::parse(&search.to_string()), search);
        }
        for state in [
            ElementState::Allowed,
            ElementState::Required,
            ElementState::Excluded,
        ] {
            assert_eq!(
                state
                    .to_string()
                    .parse::<ElementState>()
                    .unwrap_or_default(),
                state
            );
        }
    }

    #[test]
    fn an_unrecognised_spelling_falls_back_rather_than_failing() {
        // Note the two ways in: `ElementState` is reached through `FromStr`,
        // `SmilesSearchType` through an inherent `parse`. Both are infallible, so
        // `parse` cannot fail and the tests use `unwrap_or_default` only to keep
        // the comparison types lined up.
        assert_eq!(
            "maybe".parse::<ElementState>().unwrap_or_default(),
            ElementState::Allowed,
            "the default is the absence of a constraint"
        );
        assert_eq!(SmilesSearchType::parse("fuzzy"), SmilesSearchType::Exact);
    }
}

#[cfg(test)]
mod from_entries_tests {
    use super::*;

    fn entry(compound: &str, taxon: &str, reference: &str) -> CompoundEntry {
        CompoundEntry {
            compound_qid: compound.into(),
            taxon_qid: taxon.into(),
            reference_qid: reference.into(),
            ..CompoundEntry::default()
        }
    }

    #[test]
    fn counts_distinct_values_not_rows() {
        let rows = vec![
            entry("Q1", "Q10", "Q100"),
            entry("Q1", "Q10", "Q100"),
            entry("Q1", "Q11", "Q101"),
        ];
        let stats = DatasetStats::from_entries(&rows);
        assert_eq!(stats.n_compounds, 1, "the same compound three times");
        assert_eq!(stats.n_taxa, 2);
        assert_eq!(stats.n_references, 2);
        assert_eq!(stats.n_entries, 3, "rows are counted, not deduplicated");
        assert_eq!(stats.n_entries_unique, 2, "the repeated triple is one");
    }

    #[test]
    fn an_empty_row_contributes_no_taxon_or_reference() {
        // A compound that matched with no occurrence and no citation is still a
        // compound; counting it as a taxon or a reference would report a
        // diversity the data does not have.
        let rows = vec![entry("Q1", "", "")];
        let stats = DatasetStats::from_entries(&rows);
        assert_eq!(stats.n_compounds, 1);
        assert_eq!(stats.n_taxa, 0);
        assert_eq!(stats.n_references, 0);
        assert_eq!(stats.n_entries_unique, 1);
    }

    #[test]
    fn an_empty_slice_is_all_zero() {
        let stats = DatasetStats::from_entries(&[]);
        assert_eq!(stats.n_compounds, 0);
        assert_eq!(stats.n_entries, 0);
        assert_eq!(stats.n_entries_unique, 0);
    }

    #[test]
    fn distinct_triples_can_exceed_distinct_values() {
        // The same two compounds, two taxa and two references give four triples
        // from four values; the two counts are not interchangeable, and an
        // implementation that computed one from the other would be wrong here.
        let rows = vec![
            entry("Q1", "Q10", "Q100"),
            entry("Q1", "Q11", "Q101"),
            entry("Q2", "Q10", "Q100"),
            entry("Q2", "Q11", "Q101"),
        ];
        let stats = DatasetStats::from_entries(&rows);
        assert_eq!(stats.n_compounds, 2);
        assert_eq!(stats.n_taxa, 2);
        assert_eq!(stats.n_references, 2);
        assert_eq!(stats.n_entries_unique, 4);
    }
}
