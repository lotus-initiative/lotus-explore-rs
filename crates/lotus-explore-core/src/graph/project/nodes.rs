// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Node selection, degree ranking, deterministic spherical seeding, and label materialisation.

use std::cmp::Reverse;
use std::collections::HashMap;
use std::fmt::Write;

use super::super::model::{GraphNode, GraphScope, NodeKey, NodeKind};
use super::edges::EdgeAccum;

const MAX_TOP_HUBS: usize = 16;

#[derive(Debug, Clone, Copy)]
pub(super) struct NodeStat {
    pub(super) occurrence_count: u32,
    pub(super) sample_row: usize,
}

pub(super) fn collect_node_stats(
    set: &lotus_model::ColumnarResultSet,
    rows: &[u32],
) -> (HashMap<NodeKey, NodeStat>, [usize; 3]) {
    let mut stats: HashMap<NodeKey, NodeStat> = HashMap::new();
    let mut counts = [0usize; 3];

    let mut record = |kind: NodeKind, qid: Option<u32>, row: usize| {
        let Some(id) = qid else {
            return;
        };
        let key = NodeKey::new(kind, id);
        stats
            .entry(key)
            .and_modify(|stat| {
                stat.occurrence_count = stat.occurrence_count.saturating_add(1);
            })
            .or_insert_with(|| {
                let slot = kind.slot();
                if let Some(c) = counts.get_mut(slot) {
                    *c = c.saturating_add(1);
                }
                NodeStat {
                    occurrence_count: 1,
                    sample_row: row,
                }
            });
    };

    for &row_u32 in rows {
        let row = row_u32 as usize;
        record(NodeKind::Compound, set.compound_qid(row), row);
        record(NodeKind::Taxon, set.taxon_qid(row), row);
        record(NodeKind::Reference, set.reference_qid(row), row);
    }

    (stats, counts)
}

pub(super) fn select_scope_keys(
    scope: GraphScope,
    node_stats: &HashMap<NodeKey, NodeStat>,
    edge_map: &HashMap<(NodeKey, NodeKey), EdgeAccum>,
    degree_map: &HashMap<NodeKey, u32>,
) -> (Vec<NodeKey>, bool) {
    let sort_key = |key: NodeKey| {
        let deg = degree_map.get(&key).copied().unwrap_or(0);
        let occ = node_stats.get(&key).map_or(0, |s| s.occurrence_count);
        (Reverse(deg), Reverse(occ), key)
    };

    match scope {
        GraphScope::Overview { max_nodes } => {
            let mut ranked: Vec<NodeKey> = degree_map.keys().copied().collect();
            ranked.sort_by_key(|&key| sort_key(key));
            let capped = ranked.len() > max_nodes;
            ranked.truncate(max_nodes);
            (ranked, capped)
        }
        GraphScope::Ego {
            center,
            max_neighbors,
        } => {
            let mut neighbors: Vec<(NodeKey, u32)> = edge_map
                .iter()
                .filter_map(|(&(a, b), accum)| {
                    if a == center {
                        Some((b, accum.weight()))
                    } else if b == center {
                        Some((a, accum.weight()))
                    } else {
                        None
                    }
                })
                .collect();
            neighbors.sort_by_key(|&(other, weight)| {
                let (deg, occ, key) = sort_key(other);
                (Reverse(weight), deg, occ, key)
            });
            let capped = neighbors.len() > max_neighbors;
            neighbors.truncate(max_neighbors);

            let mut selected = Vec::with_capacity(neighbors.len().saturating_add(1));
            selected.push(center);
            selected.extend(neighbors.into_iter().map(|(k, _)| k));
            (selected, capped)
        }
    }
}

pub(super) fn build_nodes(
    set: &lotus_model::ColumnarResultSet,
    selected_keys: &[NodeKey],
    node_stats: &HashMap<NodeKey, NodeStat>,
    degree_map: &HashMap<NodeKey, u32>,
    is_ego: bool,
) -> Vec<GraphNode> {
    let total = selected_keys.len();
    let mut nodes = Vec::with_capacity(total);

    for (idx, &key) in selected_keys.iter().enumerate() {
        let stat = node_stats.get(&key).copied().unwrap_or(NodeStat {
            occurrence_count: 1,
            sample_row: 0,
        });
        let degree = degree_map.get(&key).copied().unwrap_or(0);
        let (qid_text, label, secondary) = materialize_node_text(set, key, stat.sample_row);
        let degree_clamped = u16::try_from(degree).unwrap_or(u16::MAX);
        let radius = 4.5_f32 + (f32::from(degree_clamped).ln_1p() * 2.4_f32).min(11.5_f32);
        let (init_x, init_y, init_z) = if is_ego && idx == 0 {
            (0.0_f32, 0.0_f32, 0.0_f32)
        } else {
            seeded_coords(key, idx, total, degree)
        };

        nodes.push(GraphNode {
            key,
            qid_text,
            label,
            secondary,
            degree,
            occurrence_count: stat.occurrence_count,
            sample_row: stat.sample_row,
            radius,
            is_hub: false,
            init_x,
            init_y,
            init_z,
        });
    }

    nodes
}

pub(super) fn compute_top_hubs(nodes: &[GraphNode]) -> Vec<usize> {
    let mut ranked: Vec<usize> = (0..nodes.len()).collect();
    ranked.sort_by_key(|&idx| {
        nodes.get(idx).map_or(
            (
                Reverse(0u32),
                Reverse(0u32),
                NodeKey::new(NodeKind::Compound, u32::MAX),
            ),
            |node| {
                (
                    Reverse(node.degree),
                    Reverse(node.occurrence_count),
                    node.key,
                )
            },
        )
    });
    ranked.truncate(MAX_TOP_HUBS);
    ranked
}

pub(super) fn materialize_node_text(
    set: &lotus_model::ColumnarResultSet,
    key: NodeKey,
    sample_row: usize,
) -> (String, String, Option<String>) {
    let qid_text = key.qid_text();
    match key.kind {
        NodeKind::Compound => {
            let label = set
                .compound_label(sample_row)
                .map_or_else(|| qid_text.clone(), str::to_owned);
            let formula = set.formula(sample_row).filter(|f| !f.trim().is_empty());
            let mass = set.mass(sample_row);
            let secondary = format_compound_secondary(formula, mass);
            (qid_text, label, secondary)
        }
        NodeKind::Taxon => {
            let label = set
                .taxon_label(sample_row)
                .filter(|name| !name.trim().is_empty())
                .map_or_else(|| qid_text.clone(), str::to_owned);
            (qid_text, label, None)
        }
        NodeKind::Reference => {
            let label = set
                .reference_title(sample_row)
                .filter(|t| !t.trim().is_empty())
                .or_else(|| {
                    set.reference_doi(sample_row)
                        .filter(|d| !d.trim().is_empty())
                })
                .map_or_else(|| qid_text.clone(), str::to_owned);
            let secondary = set.pub_year(sample_row).map(|year| year.to_string());
            (qid_text, label, secondary)
        }
    }
}

fn format_compound_secondary(formula: Option<&str>, mass: Option<f64>) -> Option<String> {
    match (formula, mass) {
        (Some(f), Some(m)) => {
            let mut out = String::with_capacity(f.len() + 16);
            let _ = write!(out, "{f} · {m:.2} Da");
            Some(out)
        }
        (Some(f), None) => Some(f.to_owned()),
        (None, Some(m)) => {
            let mut out = String::with_capacity(16);
            let _ = write!(out, "{m:.2} Da");
            Some(out)
        }
        (None, None) => None,
    }
}

fn seeded_coords(key: NodeKey, idx: usize, total: usize, degree: u32) -> (f32, f32, f32) {
    let raw_seed = (u64::from(key.kind.as_u8()) << 32) + u64::from(key.qid);
    let seed = splitmix64(raw_seed);
    let u0 = f32::from(u16::try_from(seed & 0xffff).unwrap_or(0)) / 65_535.0_f32;
    let u1 = f32::from(u16::try_from((seed >> 16) & 0xffff).unwrap_or(0)) / 65_535.0_f32;
    let u2 = f32::from(u16::try_from((seed >> 32) & 0xffff).unwrap_or(0)) / 65_535.0_f32;

    let rank = u16::try_from(idx.min(usize::from(u16::MAX))).unwrap_or(u16::MAX);
    let count = u16::try_from(total.clamp(1, usize::from(u16::MAX))).unwrap_or(u16::MAX);
    let t = (f32::from(rank) + 0.5_f32) / f32::from(count);
    let phi = u0.mul_add(0.35_f32, f32::from(rank) * 2.399_963_2_f32);
    let cos_theta = 2.0_f32.mul_add(-t, 1.0_f32).clamp(-1.0_f32, 1.0_f32);
    let sin_theta = cos_theta.mul_add(-cos_theta, 1.0_f32).max(0.0_f32).sqrt();

    let deg_u16 = u16::try_from(degree).unwrap_or(u16::MAX);
    let deg_factor = 1.0_f32 / f32::mul_add(f32::from(deg_u16).ln_1p(), 0.25_f32, 1.0_f32);
    let spread = f32::mul_add(f32::from(count).sqrt(), 8.0_f32, 90.0_f32);
    let jitter = 0.25_f32.mul_add(u1, 0.75_f32);
    let radius = spread * (0.72_f32 * deg_factor).mul_add(jitter, 0.28_f32);

    let x = radius * sin_theta * phi.cos();
    let y = radius * cos_theta;
    let z = radius * sin_theta * phi.sin() * 0.4_f32.mul_add(u2, 0.6_f32);
    (x, y, z)
}

const fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}
