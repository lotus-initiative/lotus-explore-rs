// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Axum route handlers for health, metrics, search, and streaming exports.

mod export;
mod prepare;
mod search;

use axum::{
    Json,
    extract::State,
    http::{StatusCode, header},
    response::Response,
};

use crate::{errors::ApiError, state::AppState, types::HealthResponse};

pub use self::export::{__path_export_file, __path_export_urls, export_file, export_urls};
#[cfg(test)]
pub use self::prepare::apply_resolved_taxon;
pub use self::search::{__path_search, search};

/// Health and cache observability endpoint (`GET /health`).
#[utoipa::path(
    get,
    path = "/health",
    responses((status = 200, description = "Service health", body = HealthResponse))
)]
pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(state.metrics.snapshot())
}

/// Prometheus metrics endpoint (`GET /metrics`).
///
/// # Errors
/// Returns [`ApiError`] only if HTTP response assembly fails.
#[utoipa::path(
    get,
    path = "/metrics",
    responses((status = 200, description = "Prometheus-style runtime metrics", body = String))
)]
pub async fn metrics(State(state): State<AppState>) -> Result<Response, ApiError> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .body(axum::body::Body::from(state.metrics.render_prometheus()))?)
}
