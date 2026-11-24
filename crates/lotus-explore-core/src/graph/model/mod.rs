// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Domain types for the interactive network graph view.

mod kinds;
mod payload;
mod starters;

pub use kinds::{
    DEFAULT_EGO_MAX_NEIGHBORS, DEFAULT_MAX_NODES, GraphEmptyReason, GraphProjection, GraphScope,
    NodeKey, NodeKind, OVERVIEW_MAX_NODE_PRESETS,
};
pub use payload::{GraphEdge, GraphModel, GraphNode, GraphPayload};
pub use starters::{NETWORK_STARTERS, NetworkStarter, NodeMatch};
