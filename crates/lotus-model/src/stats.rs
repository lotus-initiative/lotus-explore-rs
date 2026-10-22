// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::CompoundEntry;
use std::collections::HashSet;

/// `Display` for an enum whose printed form is its [`as_str`](Self::as_str).
///
/// Every enum here with an `as_str` wants this, and the two that had one written
/// it out by hand -- which `cargo dejadoc` reports: the second copy is where the
/// next edit lands without the first being looked at.
///
/// Must be a macro: no single function can implement `Display` for somebody
/// else's type. Invoked once, below [`ElementState`], because an `impl` block
/// cannot precede the type it names.
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
/// a QID, `InChIKey` or name by a lookup query, a SMILES or molfile by asking the
/// structure service. This type then decides what happens to that compound.
///
/// [`Self::Exact`] is the default because "this compound" is what typing a
/// compound asks, and it is cheap: one row, an index scan on the QID. The other
/// two are questions about *other* compounds, and both load a structure index and
/// score every candidate in it.
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
    /// Exact does not, and the query is built accordingly. The single place that says
    /// so, so builder and panel cannot pick differently.
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
#[path = "stats/tests.rs"]
mod tests;

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
#[path = "stats/spelling_tests.rs"]
mod spelling_tests;

#[cfg(test)]
#[path = "stats/from_entries_tests.rs"]
mod from_entries_tests;
