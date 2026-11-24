// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::{QueryParams, is_true_flag};
use crate::graph::{
    DEFAULT_EGO_MAX_NEIGHBORS, DEFAULT_MAX_NODES, GraphProjection, GraphScope, NodeKey,
};
use lotus_model::SearchCriteria;

/// Hard upper bound accepted for `?max_nodes=` on `/network`.
pub(super) const MAX_OVERVIEW_NODES_LIMIT: usize = 50_000;

/// Hard upper bound accepted for `?max_neighbors=` on `/network`.
pub(super) const MAX_EGO_NEIGHBORS_LIMIT: usize = 10_000;

/// Visualization parameters encoded into the `/network` URL query string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphViewParams {
    /// Requested graph projection (`Auto` by default, omitted when encoding).
    pub projection: GraphProjection,
    /// Overview node cap or 1-hop ego neighborhood focus.
    pub scope: GraphScope,
    /// `true` for 3D WebGL projection, `false` for flattened 2D mode.
    pub is_3d: bool,
    /// Whether the force simulation is paused.
    pub paused: bool,
    /// Explicitly selected node key, if any.
    pub selected: Option<NodeKey>,
}

impl Default for GraphViewParams {
    fn default() -> Self {
        Self {
            projection: GraphProjection::Auto,
            scope: GraphScope::Overview {
                max_nodes: DEFAULT_MAX_NODES,
            },
            is_3d: true,
            paused: false,
            selected: None,
        }
    }
}

impl GraphViewParams {
    /// Selected node key implied by `selected` or an active ego `focus`.
    #[must_use]
    pub const fn effective_selected(self) -> Option<NodeKey> {
        match self.selected {
            Some(key) => Some(key),
            None => match self.scope {
                GraphScope::Ego { center, .. } => Some(center),
                GraphScope::Overview { .. } => None,
            },
        }
    }
}

/// Decode `/network` visualization parameters from query parameters.
#[must_use]
pub fn parse_graph_view_from_params(params: &QueryParams) -> GraphViewParams {
    let projection = params
        .get("projection")
        .and_then(|raw| GraphProjection::parse(raw))
        .unwrap_or(GraphProjection::Auto);

    let scope = params
        .get("focus")
        .and_then(|v| NodeKey::parse_url_token(v))
        .map_or_else(
            || {
                let max_nodes = parse_clamped_positive(
                    params,
                    "max_nodes",
                    DEFAULT_MAX_NODES,
                    MAX_OVERVIEW_NODES_LIMIT,
                );
                GraphScope::Overview { max_nodes }
            },
            |center| {
                let max_neighbors = parse_clamped_positive(
                    params,
                    "max_neighbors",
                    DEFAULT_EGO_MAX_NEIGHBORS,
                    MAX_EGO_NEIGHBORS_LIMIT,
                );
                GraphScope::Ego {
                    center,
                    max_neighbors,
                }
            },
        );

    let is_3d = params
        .get("mode")
        .is_none_or(|v| !v.trim().eq_ignore_ascii_case("2d"));
    let paused = params.get("paused").is_some_and(|v| is_true_flag(v));
    let selected = params
        .get("selected")
        .and_then(|v| NodeKey::parse_url_token(v));

    GraphViewParams {
        projection,
        scope,
        is_3d,
        paused,
        selected,
    }
}

/// Serialize non-default `/network` visualization parameters into URL query pairs.
#[must_use]
pub(super) fn graph_view_query_params(view: &GraphViewParams) -> QueryParams {
    let mut params = QueryParams::new();

    if view.projection != GraphProjection::Auto {
        params.insert(
            "projection".to_string(),
            view.projection.as_str().to_string(),
        );
    }

    let ego_center = match view.scope {
        GraphScope::Overview { max_nodes } => {
            if max_nodes != DEFAULT_MAX_NODES {
                params.insert("max_nodes".to_string(), max_nodes.to_string());
            }
            None
        }
        GraphScope::Ego {
            center,
            max_neighbors,
        } => {
            params.insert("focus".to_string(), center.to_url_token());
            if max_neighbors != DEFAULT_EGO_MAX_NEIGHBORS {
                params.insert("max_neighbors".to_string(), max_neighbors.to_string());
            }
            Some(center)
        }
    };

    if !view.is_3d {
        params.insert("mode".to_string(), "2d".to_string());
    }
    if view.paused {
        params.insert("paused".to_string(), "true".to_string());
    }
    if let Some(selected) = view.selected
        && ego_center != Some(selected)
    {
        params.insert("selected".to_string(), selected.to_url_token());
    }

    params
}

/// Combine search criteria, optional `execute=true`, and `/network` visualization parameters.
#[must_use]
pub fn network_query_params(
    criteria: &SearchCriteria,
    year_max: u16,
    view: &GraphViewParams,
    execute: bool,
) -> QueryParams {
    let mut params = super::criteria_query_params(criteria, year_max);
    if execute && !params.is_empty() {
        params.insert("execute".to_string(), "true".to_string());
    }
    params.extend(graph_view_query_params(view));
    params
}

fn parse_clamped_positive(params: &QueryParams, key: &str, default: usize, max: usize) -> usize {
    params
        .get(key)
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .filter(|&n| n > 0)
        .map_or(default, |n| n.min(max))
}
