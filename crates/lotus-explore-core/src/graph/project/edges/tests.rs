// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Unit tests for canonical edge accumulation ordering.

use super::*;

#[test]
fn record_edge_canonicalizes_pair_order() {
    let comp = NodeKey::new(NodeKind::Compound, 1);
    let tax = NodeKey::new(NodeKind::Taxon, 10);
    let mut forward = HashMap::new();
    record_edge(&mut forward, comp, tax, Some(100));
    let mut reverse = HashMap::new();
    record_edge(&mut reverse, tax, comp, Some(100));

    assert!(forward.contains_key(&(comp, tax)));
    assert!(reverse.contains_key(&(comp, tax)));
}
