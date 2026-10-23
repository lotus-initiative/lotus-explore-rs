// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `search_metrics`, in their own file.

use super::*;
use std::time::Duration;

#[test]
fn add_network_accumulates_and_increments_call_count() {
    let mut m = SearchMetrics::default();
    m.add_network(Duration::from_millis(100));
    m.add_network(Duration::from_millis(200));
    assert!((m.network_ms - 300.0).abs() < 1.0);
    assert_eq!(m.sparql_calls, 2);
}

#[test]
fn add_parse_accumulates_independently() {
    let mut m = SearchMetrics::default();
    m.add_parse(Duration::from_millis(50));
    m.add_parse(Duration::from_millis(75));
    assert!((m.parse_ms - 125.0).abs() < 1.0);
    assert_eq!(m.sparql_calls, 0); // parse doesn't count calls
}
