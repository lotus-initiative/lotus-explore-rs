// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Vertex identity, projection modes, and graph scope types.

use serde::{Deserialize, Serialize};

/// Default maximum node count in overview scope.
pub const DEFAULT_MAX_NODES: usize = 5_000;

/// Selectable overview node-cap presets presented to the user.
pub const OVERVIEW_MAX_NODE_PRESETS: [usize; 3] = [1_000, 5_000, 10_000];

/// Default maximum neighbor count retained around the center in ego scope.
pub const DEFAULT_EGO_MAX_NEIGHBORS: usize = 500;

/// Entity class of a graph vertex.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    /// Chemical compound (`Q…`).
    Compound = 0,
    /// Biological taxon (`Q…`).
    Taxon = 1,
    /// Scholarly reference (`Q…`).
    Reference = 2,
}

impl NodeKind {
    /// Compact numeric tag used in flat WebGL buffers.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Compound => 0,
            Self::Taxon => 1,
            Self::Reference => 2,
        }
    }

    /// Array slot in `0..3` for kind-indexed counters.
    #[must_use]
    pub const fn slot(self) -> usize {
        match self {
            Self::Compound => 0,
            Self::Taxon => 1,
            Self::Reference => 2,
        }
    }

    /// Stable string tag used in URL parameters and controls.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Compound => "compound",
            Self::Taxon => "taxon",
            Self::Reference => "reference",
        }
    }

    /// Parse an entity kind from its string tag (trimmed, ASCII case-insensitive).
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.eq_ignore_ascii_case("compound") {
            Some(Self::Compound)
        } else if trimmed.eq_ignore_ascii_case("taxon") {
            Some(Self::Taxon)
        } else if trimmed.eq_ignore_ascii_case("reference") {
            Some(Self::Reference)
        } else {
            None
        }
    }
}

/// Compact vertex identifier backed by the entity class and numeric Wikidata QID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeKey {
    /// Entity class.
    pub kind: NodeKind,
    /// Numeric part of the Wikidata QID (`158572` for `Q158572`).
    pub qid: u32,
}

impl NodeKey {
    /// Construct a [`NodeKey`] from `kind` and numeric `qid`.
    #[must_use]
    pub const fn new(kind: NodeKind, qid: u32) -> Self {
        Self { kind, qid }
    }

    /// Format this key's Wikidata QID as `Q…`.
    #[must_use]
    pub fn qid_text(self) -> String {
        lotus_model::qid_text(self.qid)
    }

    /// Format this key as an unescaped URL query token (for example `compound-Q121802`).
    #[must_use]
    pub fn to_url_token(self) -> String {
        format!("{}-{}", self.kind.as_str(), self.qid_text())
    }

    /// Parse a URL query token such as `compound-Q121802` or `taxon:q158572`.
    #[must_use]
    pub fn parse_url_token(raw: &str) -> Option<Self> {
        let (kind_raw, qid_raw) = raw.split_once('-').or_else(|| raw.split_once(':'))?;
        let kind = NodeKind::parse(kind_raw)?;
        let qid_trimmed = qid_raw.trim();
        let digits = qid_trimmed
            .strip_prefix('Q')
            .or_else(|| qid_trimmed.strip_prefix('q'))?;
        if !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let qid = digits.parse::<u32>().ok().filter(|&n| n > 0)?;
        Some(Self::new(kind, qid))
    }
}

/// Graph projection over compound–taxon–reference occurrence rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GraphProjection {
    /// Automatically chooses the richest projection with >1 entity on both sides.
    #[default]
    Auto,
    /// Compounds linked to taxa, weighted by distinct corroborating references.
    CompoundTaxon,
    /// Compounds linked to references, weighted by distinct corroborating taxa.
    CompoundReference,
    /// Taxa linked to references, weighted by distinct corroborating compounds.
    TaxonReference,
    /// Compound ↔ Taxon, Compound ↔ Reference, and Taxon ↔ Reference links together.
    Tripartite,
}

impl GraphProjection {
    /// Stable string tag for controls and serialisation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::CompoundTaxon => "compound_taxon",
            Self::CompoundReference => "compound_reference",
            Self::TaxonReference => "taxon_reference",
            Self::Tripartite => "tripartite",
        }
    }

    /// Parse a projection from its string tag.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "compound_taxon" => Some(Self::CompoundTaxon),
            "compound_reference" => Some(Self::CompoundReference),
            "taxon_reference" => Some(Self::TaxonReference),
            "tripartite" => Some(Self::Tripartite),
            _ => None,
        }
    }

    /// Whether vertices of `kind` participate in this projection.
    #[must_use]
    pub const fn includes_kind(self, kind: NodeKind) -> bool {
        match self {
            Self::Auto | Self::Tripartite => true,
            Self::CompoundTaxon => !matches!(kind, NodeKind::Reference),
            Self::CompoundReference => !matches!(kind, NodeKind::Taxon),
            Self::TaxonReference => !matches!(kind, NodeKind::Compound),
        }
    }

    /// The single entity kind omitted by a 2-type projection, or `None` when
    /// all three kinds participate.
    #[must_use]
    pub const fn omitted_kind(self) -> Option<NodeKind> {
        match self {
            Self::Auto | Self::Tripartite => None,
            Self::CompoundTaxon => Some(NodeKind::Reference),
            Self::CompoundReference => Some(NodeKind::Taxon),
            Self::TaxonReference => Some(NodeKind::Compound),
        }
    }

    /// Next projection when the user toggles `kind` from a resolved `effective`
    /// projection.
    #[must_use]
    pub const fn toggle_kind(effective: Self, kind: NodeKind) -> Option<Self> {
        if !effective.includes_kind(kind) {
            return Some(Self::Tripartite);
        }
        if matches!(effective, Self::Tripartite) {
            return Some(match kind {
                NodeKind::Reference => Self::CompoundTaxon,
                NodeKind::Taxon => Self::CompoundReference,
                NodeKind::Compound => Self::TaxonReference,
            });
        }
        None
    }
}

/// Which subset of the filtered graph is projected for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphScope {
    /// Global top-hub overview bounded to `max_nodes`.
    Overview {
        /// Maximum number of vertices included in the rendered slice.
        max_nodes: usize,
    },
    /// Exact 1-hop neighborhood of `center`, placing `center` at vertex index `0`.
    Ego {
        /// Focus vertex.
        center: NodeKey,
        /// Maximum neighbor count retained around `center`.
        max_neighbors: usize,
    },
}

impl Default for GraphScope {
    fn default() -> Self {
        Self::Overview {
            max_nodes: DEFAULT_MAX_NODES,
        }
    }
}

/// Why a projected [`GraphModel`](super::GraphModel) contains no edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphEmptyReason {
    /// The filtered result set has zero rows.
    NoRows,
    /// Rows exist (for example structure-only hits), but no occurrence links connect them.
    NoOccurrenceEdges,
    /// The focused ego-graph vertex is absent from the currently filtered rows.
    CenterNotInRows,
}
