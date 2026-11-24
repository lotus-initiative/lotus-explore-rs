// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Framework-free network graph engine over a [`lotus_model::ColumnarResultSet`].
//!
//! Converts filtered occurrence rows into a bipartite or tripartite
//! compound–taxon–reference graph without copying the underlying result set,
//! resolves `Auto` projections deterministically, supports global node search
//! outside capped overviews, and builds exact [`lotus_model::SearchCriteria`]
//! pivots for any node.

mod model;
mod project;

pub use model::{
    DEFAULT_EGO_MAX_NEIGHBORS, DEFAULT_MAX_NODES, GraphEdge, GraphEmptyReason, GraphModel,
    GraphNode, GraphPayload, GraphProjection, GraphScope, NETWORK_STARTERS, NetworkStarter,
    NodeKey, NodeKind, NodeMatch, OVERVIEW_MAX_NODE_PRESETS,
};
pub use project::{build_graph, pivot_criteria_for_node, resolve_projection, search_all_nodes};

#[cfg(test)]
mod tests;
