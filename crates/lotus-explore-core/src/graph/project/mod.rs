// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Graph projection, global node search, and criteria pivot builders.

mod edges;
mod nodes;
mod search;

pub use search::search_all_nodes;

use super::model::{GraphEmptyReason, GraphModel, GraphProjection, GraphScope, NodeKey, NodeKind};
use edges::{aggregate_edges, collect_retained_edges, compute_degrees};
use nodes::{build_nodes, collect_node_stats, compute_top_hubs, select_scope_keys};
use std::collections::HashMap;

/// Resolve [`GraphProjection::Auto`] from distinct entity counts in the filtered slice.
#[must_use]
pub const fn resolve_projection(
    requested: GraphProjection,
    n_compounds: usize,
    n_taxa: usize,
    n_references: usize,
) -> GraphProjection {
    match requested {
        GraphProjection::Auto => {
            if (n_compounds > 1 && n_taxa > 1)
                || (n_compounds >= 1 && n_taxa >= 1 && n_references <= 1)
            {
                GraphProjection::CompoundTaxon
            } else if n_taxa <= 1 && n_compounds > 1 && n_references > 1 {
                GraphProjection::CompoundReference
            } else if n_compounds <= 1 && n_taxa > 1 && n_references > 1 {
                GraphProjection::TaxonReference
            } else {
                GraphProjection::CompoundTaxon
            }
        }
        other => other,
    }
}

/// Project filtered `rows` of `set` into a deterministic [`GraphModel`].
#[must_use]
pub fn build_graph(
    set: &lotus_model::ColumnarResultSet,
    rows: &[u32],
    projection: GraphProjection,
    scope: GraphScope,
) -> GraphModel {
    let (node_stats, available_kind_counts) = collect_node_stats(set, rows);
    let effective_projection = resolve_projection(
        projection,
        available_kind_counts[0],
        available_kind_counts[1],
        available_kind_counts[2],
    );

    if rows.is_empty() {
        return empty_graph(
            effective_projection,
            GraphEmptyReason::NoRows,
            available_kind_counts,
        );
    }

    if let GraphScope::Ego { center, .. } = scope
        && !node_stats.contains_key(&center)
    {
        return empty_graph(
            effective_projection,
            GraphEmptyReason::CenterNotInRows,
            available_kind_counts,
        );
    }

    let edge_map = aggregate_edges(set, rows, effective_projection);
    if edge_map.is_empty() {
        return empty_graph(
            effective_projection,
            GraphEmptyReason::NoOccurrenceEdges,
            available_kind_counts,
        );
    }

    let degree_map = compute_degrees(&edge_map);
    let total_distinct_nodes = degree_map.len();
    let total_distinct_edges = edge_map.len();

    let (selected_keys, capped) = select_scope_keys(scope, &node_stats, &edge_map, &degree_map);
    let key_to_idx: HashMap<NodeKey, u32> = selected_keys
        .iter()
        .enumerate()
        .filter_map(|(idx, &key)| u32::try_from(idx).ok().map(|i| (key, i)))
        .collect();

    let edges = collect_retained_edges(&edge_map, &key_to_idx);
    let is_ego = matches!(scope, GraphScope::Ego { .. });
    let mut nodes = build_nodes(set, &selected_keys, &node_stats, &degree_map, is_ego);

    let top_hubs = compute_top_hubs(&nodes);
    for &hub_idx in &top_hubs {
        if let Some(node) = nodes.get_mut(hub_idx) {
            node.is_hub = true;
        }
    }

    let mut out_kind_counts = [0usize; 3];
    for node in &nodes {
        let slot = node.key.kind.slot();
        if let Some(count) = out_kind_counts.get_mut(slot) {
            *count = count.saturating_add(1);
        }
    }

    GraphModel {
        nodes,
        edges,
        effective_projection,
        total_distinct_nodes,
        total_distinct_edges,
        capped,
        empty_reason: None,
        top_hubs,
        kind_counts: out_kind_counts,
        available_kind_counts,
    }
}

/// Build an exact [`lotus_model::SearchCriteria`] pivot for exploring one entity in LOTUS.
#[must_use]
pub fn pivot_criteria_for_node(
    kind: NodeKind,
    qid: &str,
    year_max: u16,
) -> lotus_model::SearchCriteria {
    let mut criteria = lotus_model::SearchCriteria::up_to_year(year_max);
    let trimmed = qid.trim();
    match kind {
        NodeKind::Compound => {
            trimmed.clone_into(&mut criteria.structure);
            "*".clone_into(&mut criteria.taxon);
        }
        NodeKind::Taxon => {
            trimmed.clone_into(&mut criteria.taxon);
        }
        NodeKind::Reference => {
            trimmed.clone_into(&mut criteria.reference);
        }
    }
    criteria
}

const fn empty_graph(
    effective_projection: GraphProjection,
    reason: GraphEmptyReason,
    available_kind_counts: [usize; 3],
) -> GraphModel {
    GraphModel {
        nodes: Vec::new(),
        edges: Vec::new(),
        effective_projection,
        total_distinct_nodes: 0,
        total_distinct_edges: 0,
        capped: false,
        empty_reason: Some(reason),
        top_hubs: Vec::new(),
        kind_counts: [0; 3],
        available_kind_counts,
    }
}
