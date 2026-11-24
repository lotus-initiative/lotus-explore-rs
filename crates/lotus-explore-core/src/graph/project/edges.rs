// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Undirected weighted edge aggregation and degree calculation.

use std::collections::{HashMap, HashSet};

use super::super::model::{GraphEdge, GraphProjection, NodeKey, NodeKind};

#[derive(Debug, Default)]
pub(super) struct EdgeAccum {
    occurrences: u32,
    corroborators: HashSet<u32>,
}

impl EdgeAccum {
    fn record(&mut self, corroborator: Option<u32>) {
        self.occurrences = self.occurrences.saturating_add(1);
        if let Some(id) = corroborator {
            self.corroborators.insert(id);
        }
    }

    pub(super) fn weight(&self) -> u32 {
        if self.corroborators.is_empty() {
            self.occurrences
        } else {
            u32::try_from(self.corroborators.len()).unwrap_or(u32::MAX)
        }
    }
}

pub(super) fn aggregate_edges(
    set: &lotus_model::ColumnarResultSet,
    rows: &[u32],
    projection: GraphProjection,
) -> HashMap<(NodeKey, NodeKey), EdgeAccum> {
    let mut edge_map: HashMap<(NodeKey, NodeKey), EdgeAccum> = HashMap::new();

    for &row_u32 in rows {
        let row = row_u32 as usize;
        let c = set.compound_qid(row);
        let t = set.taxon_qid(row);
        let r = set.reference_qid(row);

        match projection {
            GraphProjection::Auto | GraphProjection::CompoundTaxon => {
                if let (Some(c_id), Some(t_id)) = (c, t) {
                    record_edge(
                        &mut edge_map,
                        NodeKey::new(NodeKind::Compound, c_id),
                        NodeKey::new(NodeKind::Taxon, t_id),
                        r,
                    );
                }
            }
            GraphProjection::CompoundReference => {
                if let (Some(c_id), Some(r_id)) = (c, r) {
                    record_edge(
                        &mut edge_map,
                        NodeKey::new(NodeKind::Compound, c_id),
                        NodeKey::new(NodeKind::Reference, r_id),
                        t,
                    );
                }
            }
            GraphProjection::TaxonReference => {
                if let (Some(t_id), Some(r_id)) = (t, r) {
                    record_edge(
                        &mut edge_map,
                        NodeKey::new(NodeKind::Taxon, t_id),
                        NodeKey::new(NodeKind::Reference, r_id),
                        c,
                    );
                }
            }
            GraphProjection::Tripartite => {
                if let (Some(c_id), Some(t_id)) = (c, t) {
                    record_edge(
                        &mut edge_map,
                        NodeKey::new(NodeKind::Compound, c_id),
                        NodeKey::new(NodeKind::Taxon, t_id),
                        r,
                    );
                }
                if let (Some(c_id), Some(r_id)) = (c, r) {
                    record_edge(
                        &mut edge_map,
                        NodeKey::new(NodeKind::Compound, c_id),
                        NodeKey::new(NodeKind::Reference, r_id),
                        t,
                    );
                }
                if let (Some(t_id), Some(r_id)) = (t, r) {
                    record_edge(
                        &mut edge_map,
                        NodeKey::new(NodeKind::Taxon, t_id),
                        NodeKey::new(NodeKind::Reference, r_id),
                        c,
                    );
                }
            }
        }
    }

    edge_map
}

fn record_edge(
    edge_map: &mut HashMap<(NodeKey, NodeKey), EdgeAccum>,
    a: NodeKey,
    b: NodeKey,
    corroborator: Option<u32>,
) {
    let pair = if a <= b { (a, b) } else { (b, a) };
    edge_map.entry(pair).or_default().record(corroborator);
}

pub(super) fn compute_degrees(
    edge_map: &HashMap<(NodeKey, NodeKey), EdgeAccum>,
) -> HashMap<NodeKey, u32> {
    let mut degrees: HashMap<NodeKey, u32> = HashMap::new();
    for &(a, b) in edge_map.keys() {
        *degrees.entry(a).or_insert(0) += 1;
        *degrees.entry(b).or_insert(0) += 1;
    }
    degrees
}

pub(super) fn collect_retained_edges(
    edge_map: &HashMap<(NodeKey, NodeKey), EdgeAccum>,
    key_to_idx: &HashMap<NodeKey, u32>,
) -> Vec<GraphEdge> {
    let mut edges: Vec<GraphEdge> = edge_map
        .iter()
        .filter_map(|(&(a, b), accum)| {
            let src = *key_to_idx.get(&a)?;
            let dst = *key_to_idx.get(&b)?;
            let (source, target) = if src <= dst { (src, dst) } else { (dst, src) };
            Some(GraphEdge {
                source,
                target,
                weight: accum.weight(),
            })
        })
        .collect();
    edges.sort_by_key(|edge| (edge.source, edge.target));
    edges
}
