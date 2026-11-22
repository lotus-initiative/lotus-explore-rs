// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Domain types for the interactive network graph view.

use serde::{Deserialize, Serialize};
use std::cmp::Reverse;

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
        match raw.trim() {
            "auto" => Some(Self::Auto),
            "compound_taxon" => Some(Self::CompoundTaxon),
            "compound_reference" => Some(Self::CompoundReference),
            "taxon_reference" => Some(Self::TaxonReference),
            "tripartite" => Some(Self::Tripartite),
            _ => None,
        }
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

/// Why a projected [`GraphModel`] contains no edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphEmptyReason {
    /// The filtered result set has zero rows.
    NoRows,
    /// Rows exist (for example structure-only hits), but no occurrence links connect them.
    NoOccurrenceEdges,
    /// The focused ego-graph vertex is absent from the currently filtered rows.
    CenterNotInRows,
}

/// One vertex in the projected graph.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphNode {
    /// Numeric entity key.
    pub key: NodeKey,
    /// Canonical `Q…` identifier.
    pub qid_text: String,
    /// Display label (falls back to `qid_text` when unnamed).
    pub label: String,
    /// Optional concise secondary datum (formula/mass or publication year).
    pub secondary: Option<String>,
    /// Degree over the full filtered result set before overview capping.
    pub degree: u32,
    /// Number of filtered occurrence rows mentioning this entity.
    pub occurrence_count: u32,
    /// Index of one filtered row referencing this entity for zero-copy metadata lookup.
    pub sample_row: usize,
    /// Base WebGL point radius derived monotonically from `degree`.
    pub radius: f32,
    /// Whether this vertex ranks among the graph's top hubs for priority labeling.
    pub is_hub: bool,
    /// Deterministic initial X coordinate.
    pub init_x: f32,
    /// Deterministic initial Y coordinate.
    pub init_y: f32,
    /// Deterministic initial Z coordinate.
    pub init_z: f32,
}

/// One undirected weighted link between two retained vertex indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphEdge {
    /// Index into [`GraphModel::nodes`].
    pub source: u32,
    /// Index into [`GraphModel::nodes`].
    pub target: u32,
    /// Corroboration weight (distinct third-axis entities, or occurrence count if absent).
    pub weight: u32,
}

/// Compact flat-array payload transferred to the WebGL2 renderer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphPayload {
    /// Node kinds (`0` = Compound, `1` = Taxon, `2` = Reference), length `N`.
    pub kinds: Vec<u8>,
    /// Node base radii, length `N`.
    pub radii: Vec<f32>,
    /// Full-graph node degrees, length `N`.
    pub degrees: Vec<u32>,
    /// Interleaved initial coordinates `[x0, y0, z0, ...]`, length `3 * N`.
    pub init_pos: Vec<f32>,
    /// Node display labels, length `N`.
    pub labels: Vec<String>,
    /// Interleaved edge endpoint pairs `[src0, dst0, ...]`, length `2 * E`.
    pub edges: Vec<u32>,
    /// Edge corroboration weights, length `E`.
    pub edge_weights: Vec<u32>,
    /// Per-node hub priority rank (`1..=K` for top hubs, `0` otherwise), length `N`.
    pub hub_index: Vec<usize>,
    /// Resolved projection used to construct this payload.
    pub effective_projection: GraphProjection,
    /// Total distinct participating nodes before scope capping.
    pub total_distinct_nodes: usize,
    /// Total distinct edges before scope capping.
    pub total_distinct_edges: usize,
    /// Whether overview or ego capping omitted entities or neighbors.
    pub capped: bool,
    /// Reason when no graph can be rendered.
    pub empty_reason: Option<GraphEmptyReason>,
    /// Indices of the top connected vertices in descending degree order.
    pub top_hubs: Vec<usize>,
}

/// Complete projected graph used by both the Dioxus Inspector and WebGL2 bridge.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphModel {
    /// Retained vertices.
    pub nodes: Vec<GraphNode>,
    /// Retained edges whose endpoints both belong to `nodes`.
    pub edges: Vec<GraphEdge>,
    /// Resolved projection (`Auto` never appears here).
    pub effective_projection: GraphProjection,
    /// Total distinct participating nodes before capping.
    pub total_distinct_nodes: usize,
    /// Total distinct edges before capping.
    pub total_distinct_edges: usize,
    /// Whether capping dropped any participating node or neighbor.
    pub capped: bool,
    /// Empty-state cause when `nodes` cannot form a graph.
    pub empty_reason: Option<GraphEmptyReason>,
    /// Indices into `nodes` of the highest-degree entities.
    pub top_hubs: Vec<usize>,
    /// Counts of retained vertices per [`NodeKind`] (`[Compound, Taxon, Reference]`).
    pub kind_counts: [usize; 3],
}

impl GraphModel {
    /// Convert this model into the compact flat payload expected by `LotusGraphGL.setGraph`.
    #[must_use]
    pub fn to_payload(&self) -> GraphPayload {
        let n = self.nodes.len();
        let e = self.edges.len();
        let mut kinds = Vec::with_capacity(n);
        let mut radii = Vec::with_capacity(n);
        let mut degrees = Vec::with_capacity(n);
        let mut init_pos = Vec::with_capacity(n.saturating_mul(3));
        let mut labels = Vec::with_capacity(n);
        let mut hub_index = vec![0usize; n];

        for node in &self.nodes {
            kinds.push(node.key.kind.as_u8());
            radii.push(node.radius);
            degrees.push(node.degree);
            init_pos.push(node.init_x);
            init_pos.push(node.init_y);
            init_pos.push(node.init_z);
            labels.push(node.label.clone());
        }

        for (rank, &idx) in self.top_hubs.iter().enumerate() {
            if let Some(slot) = hub_index.get_mut(idx) {
                *slot = rank + 1;
            }
        }

        let mut edges = Vec::with_capacity(e.saturating_mul(2));
        let mut edge_weights = Vec::with_capacity(e);
        for edge in &self.edges {
            edges.push(edge.source);
            edges.push(edge.target);
            edge_weights.push(edge.weight);
        }

        GraphPayload {
            kinds,
            radii,
            degrees,
            init_pos,
            labels,
            edges,
            edge_weights,
            hub_index,
            effective_projection: self.effective_projection,
            total_distinct_nodes: self.total_distinct_nodes,
            total_distinct_edges: self.total_distinct_edges,
            capped: self.capped,
            empty_reason: self.empty_reason,
            top_hubs: self.top_hubs.clone(),
        }
    }

    /// Find the index of `key` in `nodes`, if retained.
    #[must_use]
    pub fn index_of(&self, key: NodeKey) -> Option<usize> {
        self.nodes.iter().position(|node| node.key == key)
    }

    /// Number of retained vertices of a given [`NodeKind`].
    #[must_use]
    pub const fn kind_count(&self, kind: NodeKind) -> usize {
        match kind {
            NodeKind::Compound => self.kind_counts[0],
            NodeKind::Taxon => self.kind_counts[1],
            NodeKind::Reference => self.kind_counts[2],
        }
    }

    /// Direct neighbors of `node_idx` sorted by descending edge weight, then degree, then key.
    #[must_use]
    pub fn neighbors_of(&self, node_idx: usize) -> Vec<(usize, u32)> {
        let Ok(target_u32) = u32::try_from(node_idx) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for edge in &self.edges {
            if edge.source == target_u32 {
                out.push((edge.target as usize, edge.weight));
            } else if edge.target == target_u32 {
                out.push((edge.source as usize, edge.weight));
            }
        }
        out.sort_by_key(|&(neighbor_idx, weight)| {
            let (deg, key) = self
                .nodes
                .get(neighbor_idx)
                .map_or((0u32, NodeKey::new(NodeKind::Compound, u32::MAX)), |node| {
                    (node.degree, node.key)
                });
            (Reverse(weight), Reverse(deg), key)
        });
        out
    }
}

/// Entity match returned by [`super::search_all_nodes`] across the full filtered set.
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
