// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Characterization tests pinning the HTTP API JSON schemas, aliases, and
//! error/header contracts for `lotus-server`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "characterization test assertions on router responses"
)]

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;
use utoipa::OpenApi;

use super::{
    ApiDoc, build_router,
    cache_key::build_export_cache_key,
    config::AppConfig,
    dto::{ExportUrlResponse, SearchRequest},
    state::{AppState, export_cache_put},
};

fn test_config() -> AppConfig {
    AppConfig::from_provider(|_| None).expect("default config should parse")
}

#[tokio::test]
async fn health_and_openapi_schemas_pin_expected_fields() {
    let config = test_config();
    let app = build_router(config.max_body_bytes, &config, AppState::new(&config));

    let health_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("health request"),
        )
        .await
        .expect("health response");
    assert_eq!(
        health_resp.status(),
        StatusCode::OK,
        "health status must be 200"
    );
    let health_bytes = to_bytes(health_resp.into_body(), usize::MAX)
        .await
        .expect("health bytes");
    let health_json: serde_json::Value =
        serde_json::from_slice(&health_bytes).expect("health json");
    for key in [
        "status",
        "uptime_secs",
        "search_cache_hits",
        "search_cache_misses",
        "search_inflight_waits",
        "search_upstream_hits",
        "export_cache_hits",
        "export_cache_misses",
        "export_inflight_waits",
        "export_upstream_hits",
        "overload_rejections",
        "upstream_shed",
        "request_timeouts",
    ] {
        assert!(
            health_json.get(key).is_some(),
            "health response missing expected key `{key}`"
        );
    }

    let doc = ApiDoc::openapi();
    let schemas = &doc.components.as_ref().expect("openapi components").schemas;
    for schema in [
        "HealthResponse",
        "SearchRequest",
        "SearchResponse",
        "SearchStats",
        "RowDto",
        "ExportUrlResponse",
        "ErrorResponse",
        "ApiSmilesSearchType",
        "ApiElementState",
    ] {
        assert!(
            schemas.contains_key(schema),
            "OpenAPI schema missing `{schema}`"
        );
    }
}

#[test]
fn search_request_accepts_both_canonical_and_legacy_aliases() {
    let legacy = r#"{
        "taxon": "*",
        "smiles": "CCO",
        "smiles_search_type": "similarity",
        "smiles_threshold": 0.7
    }"#;
    let modern = r#"{
        "taxon": "*",
        "structure": "CCO",
        "structure_search": "similarity",
        "structure_threshold": 0.7
    }"#;

    let a: SearchRequest = serde_json::from_str(legacy).expect("legacy aliases deserialize");
    let b: SearchRequest = serde_json::from_str(modern).expect("modern aliases deserialize");
    assert_eq!(a.smiles, b.smiles, "smiles vs structure alias mismatch");
    assert_eq!(
        a.similarity_threshold, b.similarity_threshold,
        "smiles_threshold vs structure_threshold mismatch"
    );
}

#[tokio::test]
async fn export_file_missing_cache_key_returns_400_json_error() {
    let config = test_config();
    let state = AppState::new(&config);
    let key = build_export_cache_key("SELECT ?c WHERE {}");
    export_cache_put(
        &state,
        key,
        ExportUrlResponse {
            query: "SELECT ?c WHERE {}".into(),
            csv_url: "/v1/export-file/k/csv".into(),
            json_url: "/v1/export-file/k/json".into(),
            rdf_url: "/v1/export-file/k/ttl".into(),
            csv_gz_url: "/v1/export-file/k/csv".into(),
            json_gz_url: "/v1/export-file/k/json".into(),
            rdf_gz_url: "/v1/export-file/k/ttl".into(),
        },
    );
    let app = build_router(config.max_body_bytes, &config, state);

    let miss_resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/export-file/export:missing/csv")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(
        miss_resp.status(),
        StatusCode::BAD_REQUEST,
        "missing cache key must return 400"
    );
    assert_eq!(
        miss_resp
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.starts_with("application/json")),
        Some(true),
        "error response must be application/json"
    );
    let err_bytes = to_bytes(miss_resp.into_body(), usize::MAX)
        .await
        .expect("error body");
    let err_json: serde_json::Value = serde_json::from_slice(&err_bytes).expect("error json");
    assert!(
        err_json.get("error").and_then(|v| v.as_str()).is_some(),
        "error payload must have string `error` key"
    );
}
