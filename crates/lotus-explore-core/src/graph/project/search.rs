// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Global node search across all distinct participating entities.

use std::cmp::Reverse;

use super::super::model::{NodeKey, NodeKind, NodeMatch};
use super::nodes::{collect_node_stats, materialize_node_text};

/// Search across *all* distinct entities in `rows`, including those outside a capped overview.
#[must_use]
pub fn search_all_nodes(
    set: &lotus_model::ColumnarResultSet,
    rows: &[u32],
    query: &str,
    limit: usize,
) -> Vec<NodeMatch> {
    let needle = lotus_model::folded(query);
    if needle.is_empty() || limit == 0 {
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
