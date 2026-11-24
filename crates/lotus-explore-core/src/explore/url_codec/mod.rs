// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Pure URL codec for explore state.

use std::collections::BTreeMap;

mod criteria;
mod encode;
mod network;
mod startup;

pub use criteria::{criteria_query_params, parse_criteria_from_params};
pub use encode::{
    build_network_shareable_url, build_shareable_url, encode_query_params, parse_query_string,
};
pub use network::{
    GraphViewParams, MAX_EGO_NEIGHBORS_LIMIT, MAX_OVERVIEW_NODES_LIMIT, graph_view_query_params,
    network_query_params, parse_graph_view_from_params,
};
pub use startup::{InitialDownloadState, parse_startup_action_from_params};

/// The URL query parameters of a search, keyed by parameter name.
pub type QueryParams = BTreeMap<String, String>;

/// Test whether a URL query-parameter value represents a boolean true flag.
///
/// Accepts `"1"`, `"true"`, `"yes"`, and `"on"` (case-insensitive, trimmed).
/// All other values — including absent keys — are treated as false.
#[must_use]
pub fn is_true_flag(v: &str) -> bool {
    let t = v.trim();
    t == "1"
        || t.eq_ignore_ascii_case("true")
        || t.eq_ignore_ascii_case("yes")
        || t.eq_ignore_ascii_case("on")
}

#[cfg(test)]
mod tests;
