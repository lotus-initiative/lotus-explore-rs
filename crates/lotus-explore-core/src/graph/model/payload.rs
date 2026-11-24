// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Graph model, flat WebGL2 payload buffer projection, and neighborhood queries.

use std::cmp::Reverse;

use super::types::{GraphEmptyReason, GraphProjection, NodeKey, NodeKind};

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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// Counts of distinct entities per [`NodeKind`] across all filtered rows
    /// before projection or overview capping.
    pub available_kind_counts: [usize; 3],
}

const fn slot_count(counts: [usize; 3], kind: NodeKind) -> usize {
    match kind {
        NodeKind::Compound => counts[0],
        NodeKind::Taxon => counts[1],
        NodeKind::Reference => counts[2],
    }
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
        slot_count(self.kind_counts, kind)
    }

    /// Number of distinct entities of `kind` present across the filtered rows
    /// before projection or overview capping.
    #[must_use]
    pub const fn available_kind_count(&self, kind: NodeKind) -> usize {
        slot_count(self.available_kind_counts, kind)
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
