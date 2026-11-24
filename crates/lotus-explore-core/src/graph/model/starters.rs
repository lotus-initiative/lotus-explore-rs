// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Curated `/network` starters and global node search match type.

use super::types::{NodeKey, NodeKind};

/// Entity match returned by [`super::super::search_all_nodes`] across the full filtered set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeMatch {
    /// Numeric entity key.
    pub key: NodeKey,
    /// Canonical `Q…` text.
    pub qid_text: String,
    /// Display label.
    pub label: String,
    /// Occurrence count across filtered rows.
    pub occurrence_count: u32,
    /// Sample filtered row for metadata inspection.
    pub sample_row: usize,
}

/// Curated starter query shown on `/network` before any search has run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkStarter {
    /// Stable identifier used for UI copy and testing.
    pub id: &'static str,
    /// Primary entity focus of the starter.
    pub kind: NodeKind,
    /// Taxon input (`"*"` for compound-focused starters).
    pub taxon: &'static str,
    /// Structure/compound input (empty for taxon-focused starters).
    pub structure: &'static str,
    /// Structure matching mode.
    pub structure_search: lotus_model::SmilesSearchType,
    /// Reference input (empty unless reference-focused).
    pub reference: &'static str,
}

impl NetworkStarter {
    /// Convert this starter definition into validated [`lotus_model::SearchCriteria`].
    #[must_use]
    pub fn to_criteria(self, year_max: u16) -> lotus_model::SearchCriteria {
        let mut criteria = lotus_model::SearchCriteria::up_to_year(year_max);
        self.taxon.clone_into(&mut criteria.taxon);
        self.structure.clone_into(&mut criteria.structure);
        criteria.structure_search = self.structure_search;
        self.reference.clone_into(&mut criteria.reference);
        criteria
    }
}

/// Four curated one-click starter definitions built purely as `SearchCriteria` parameters.
pub const NETWORK_STARTERS: [NetworkStarter; 4] = [
    NetworkStarter {
        id: "beta_sitosterol",
        kind: NodeKind::Compound,
        taxon: "*",
        structure: "Q121802",
        structure_search: lotus_model::SmilesSearchType::Exact,
        reference: "",
    },
    NetworkStarter {
        id: "gentiana_genus",
        kind: NodeKind::Taxon,
        taxon: "Gentiana",
        structure: "",
        structure_search: lotus_model::SmilesSearchType::Exact,
        reference: "",
    },
    NetworkStarter {
        id: "gentiana_lutea",
        kind: NodeKind::Taxon,
        taxon: "Q158572",
        structure: "",
        structure_search: lotus_model::SmilesSearchType::Exact,
        reference: "",
    },
    NetworkStarter {
        id: "indole_substructure",
        kind: NodeKind::Compound,
        taxon: "*",
        structure: "c1ccc2[nH]ccc2c1",
        structure_search: lotus_model::SmilesSearchType::Substructure,
        reference: "",
    },
];
