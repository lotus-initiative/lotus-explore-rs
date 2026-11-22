// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Graph projection, global node search, and criteria pivot builders.

use super::model::{
    GraphEdge, GraphEmptyReason, GraphModel, GraphNode, GraphProjection, GraphScope, NodeKey,
    NodeKind, NodeMatch,
};
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

const MAX_TOP_HUBS: usize = 16;

#[derive(Debug, Clone, Copy)]
struct NodeStat {
    occurrence_count: u32,
    sample_row: usize,
}

#[derive(Debug, Default)]
struct EdgeAccum {
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

    fn weight(&self) -> u32 {
        if self.corroborators.is_empty() {
            self.occurrences
        } else {
            u32::try_from(self.corroborators.len()).unwrap_or(u32::MAX)
        }
    }
}

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
    let (node_stats, kind_counts) = collect_node_stats(set, rows);
    let effective_projection =
        resolve_projection(projection, kind_counts[0], kind_counts[1], kind_counts[2]);

    if rows.is_empty() {
        return empty_graph(effective_projection, GraphEmptyReason::NoRows);
    }

    if let GraphScope::Ego { center, .. } = scope
        && !node_stats.contains_key(&center)
    {
        return empty_graph(effective_projection, GraphEmptyReason::CenterNotInRows);
    }

    let edge_map = aggregate_edges(set, rows, effective_projection);
    if edge_map.is_empty() {
        return empty_graph(effective_projection, GraphEmptyReason::NoOccurrenceEdges);
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
    }
}

/// Search across *all* distinct entities in `rows`, including those outside a capped overview.
#[must_use]
pub fn search_all_nodes(
    set: &lotus_model::ColumnarResultSet,
    rows: &[u32],
    query: &str,
    limit: usize,
) -> Vec<NodeMatch> {
    let needle = lotus_model::folded(query);
    if needle.is_empty() || limit == 0 || rows.is_empty() {
        return Vec::new();
    }

    let (node_stats, _) = collect_node_stats(set, rows);
    let mut qid_buf = [0u8; 16];
    let mut candidates: Vec<(bool, NodeMatch)> = Vec::new();

    for (key, stat) in node_stats {
        let qid_slice = qid_str(key.qid, &mut qid_buf);
        let (matched, starts_with) =
            entity_matches_needle(set, key, stat.sample_row, qid_slice, &needle);
        if !matched {
            continue;
        }
        let (qid_text, label, _) = materialize_node_text(set, key, stat.sample_row);
        let label_starts = starts_with || starts_with_folded(&label, &needle);
        candidates.push((
            label_starts,
            NodeMatch {
                key,
                qid_text,
                label,
                occurrence_count: stat.occurrence_count,
                sample_row: stat.sample_row,
            },
        ));
    }

    candidates.sort_by_key(|(starts, m)| (Reverse(*starts), Reverse(m.occurrence_count), m.key));
    candidates.truncate(limit);
    candidates.into_iter().map(|(_, m)| m).collect()
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
    }
}

fn collect_node_stats(
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

fn aggregate_edges(
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

fn compute_degrees(edge_map: &HashMap<(NodeKey, NodeKey), EdgeAccum>) -> HashMap<NodeKey, u32> {
    let mut degrees: HashMap<NodeKey, u32> = HashMap::new();
    for &(a, b) in edge_map.keys() {
        *degrees.entry(a).or_insert(0) += 1;
        *degrees.entry(b).or_insert(0) += 1;
    }
    degrees
}

fn select_scope_keys(
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

fn collect_retained_edges(
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

fn build_nodes(
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

fn compute_top_hubs(nodes: &[GraphNode]) -> Vec<usize> {
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

fn materialize_node_text(
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
    let raw_seed = (u64::from(key.kind.as_u8()) << 32) | u64::from(key.qid);
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

fn qid_str(qid: u32, buf: &mut [u8; 16]) -> &str {
    let len = lotus_model::write_qid(qid, buf);
    buf.get(..len)
        .and_then(|slice| std::str::from_utf8(slice).ok())
        .unwrap_or_default()
}

fn entity_matches_needle(
    set: &lotus_model::ColumnarResultSet,
    key: NodeKey,
    sample_row: usize,
    qid_slice: &str,
    needle: &[char],
) -> (bool, bool) {
    if starts_with_folded(qid_slice, needle) {
        return (true, true);
    }
    let qid_contains = lotus_model::contains_folded(qid_slice, needle);

    let text_fields: [Option<&str>; 3] = match key.kind {
        NodeKind::Compound => [
            set.compound_label(sample_row),
            set.inchikey(sample_row),
            set.formula(sample_row),
        ],
        NodeKind::Taxon => [set.taxon_label(sample_row), None, None],
        NodeKind::Reference => [
            set.reference_title(sample_row),
            set.reference_doi(sample_row),
            None,
        ],
    };

    let mut matched = qid_contains;
    let mut starts_with = false;
    for field in text_fields.into_iter().flatten() {
        if starts_with_folded(field, needle) {
            return (true, true);
        }
        if !matched && lotus_model::contains_folded(field, needle) {
            matched = true;
            starts_with = false;
        }
    }
    (matched, starts_with)
}

fn starts_with_folded(haystack: &str, needle: &[char]) -> bool {
    let mut chars = haystack.chars().flat_map(char::to_lowercase);
    needle
        .iter()
        .all(|&expected| chars.next() == Some(expected))
}
