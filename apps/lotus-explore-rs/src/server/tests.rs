// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::indexing_slicing)]
#![allow(clippy::panic)]

use std::{
    collections::HashMap,
    fs,
    time::{Duration, Instant},
};

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;
use utoipa::OpenApi;

use crate::server::{
    ApiDoc, build_router,
    config::AppConfig,
    query_logic::{apply_request, normalized_structure_input},
    state::{
        AppState, CachedExportResponse, export_inflight_cell, prune_cache, search_inflight_cell,
    },
    types::{ExportUrlResponse, SearchRequest},
};
use lotus_query::{self, ExportFormat};

fn map_provider(values: &[(&str, &str)]) -> HashMap<String, String> {
    values
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn test_config() -> AppConfig {
    AppConfig::from_provider(|_| None).expect("test config")
}

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body bytes");
    serde_json::from_slice(&bytes).expect("json body")
}

fn content_type_header(response: &axum::response::Response) -> String {
    response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

#[test]
fn supports_u16_formula_ranges() {
    let req = SearchRequest {
        taxon: Some("*".to_string()),
        structure: None,
        structure_search: None,
        structure_threshold: None,
        mass_min: None,
        mass_max: None,
        year_min: None,
        year_max: None,
        formula_exact: None,
        c_min: Some(1),
        c_max: Some(300),
        h_min: None,
        h_max: None,
        n_min: None,
        n_max: None,
        o_min: None,
        o_max: None,
        p_min: None,
        p_max: None,
        s_min: None,
        s_max: None,
        f_state: None,
        cl_state: None,
        br_state: None,
        i_state: None,
        limit: None,
        include_counts: None,
    };

    let c = apply_request(&req).expect("valid criteria");
    assert_eq!(c.c_max, 300);
}

#[test]
fn config_uses_safe_defaults() {
    let env = HashMap::<String, String>::new();
    let cfg = AppConfig::from_provider(|name| env.get(name).cloned()).expect("valid config");
    assert_eq!(cfg.host, "127.0.0.1");
    assert_eq!(cfg.port, 8787);
    assert_eq!(cfg.default_limit, 500);
    assert_eq!(cfg.request_timeout, Duration::from_secs(45));
    assert_eq!(cfg.max_concurrency, 256);
    assert_eq!(cfg.max_body_bytes, 1_048_576);
    assert!(cfg.cors_allowed_origins.is_none());
}

#[test]
fn config_reads_performance_tunables() {
    let env = map_provider(&[
        ("REQUEST_TIMEOUT_MS", "120000"),
        ("MAX_CONCURRENCY", "512"),
        ("MAX_BODY_BYTES", "2097152"),
    ]);
    let cfg = AppConfig::from_provider(|name| env.get(name).cloned()).expect("valid config");
    assert_eq!(cfg.request_timeout, Duration::from_mins(2));
    assert_eq!(cfg.max_concurrency, 512);
    assert_eq!(cfg.max_body_bytes, 2_097_152);
}

#[test]
fn config_rejects_invalid_port() {
    let env = map_provider(&[("PORT", "abc")]);
    let err = AppConfig::from_provider(|name| env.get(name).cloned())
        .expect_err("invalid PORT should fail");
    assert!(err.contains("PORT"));
}

#[test]
fn production_requires_explicit_cors_allowlist() {
    let env = map_provider(&[("APP_ENV", "production")]);
    let err = AppConfig::from_provider(|name| env.get(name).cloned())
        .expect_err("production without CORS origins should fail");
    assert!(err.contains("CORS_ALLOWED_ORIGINS"));
}

#[test]
fn parses_comma_separated_cors_origins() {
    let env = map_provider(&[(
        "CORS_ALLOWED_ORIGINS",
        "https://api.example.org, http://localhost:5173",
    )]);
    let cfg = AppConfig::from_provider(|name| env.get(name).cloned()).expect("valid config");
    assert_eq!(cfg.cors_allowed_origins.as_ref().map(Vec::len), Some(2));
}

#[test]
fn normalized_structure_preserves_multiline_molfile() {
    let molfile = "\n  Mrv\n\n  0  0  0  0  0  0            999 V3000\nM  END\n";
    let normalized = normalized_structure_input(molfile);
    assert!(normalized.starts_with('\n'));
    assert!(normalized.contains("V3000"));
}

#[test]
fn rdf_export_url_uses_construct_query_with_normalized_formula_binding() {
    let select = lotus_query::compounds_by_taxon_query("Q2382443");
    let url = export::qlever_export_url(&select, ExportFormat::Rdf);

    assert!(url.contains("action=turtle_export"));

    let query_param = url
        .split("query=")
        .nth(1)
        .and_then(|tail| tail.split('&').next())
        .expect("query param");
    let decoded = urlencoding::decode(query_param)
        .expect("decode query")
        .into_owned();

    assert!(decoded.contains("?c wdt:P274 ?compound_formula ."));
    assert!(decoded.contains("AS ?compound_formula"));
    assert!(decoded.contains("STR(?compound_formula_raw)"));
}

#[test]
fn prune_cache_removes_oldest_when_over_capacity() {
    let mut cache = HashMap::from([
        (
            "a".to_string(),
            CachedExportResponse {
                inserted_at: Instant::now().checked_sub(Duration::from_secs(30)).unwrap(),
                value: ExportUrlResponse {
                    query: "a".into(),
                    csv_url: "a".into(),
                    json_url: "a".into(),
                    rdf_url: "a".into(),
                    csv_gz_url: "a".into(),
                    json_gz_url: "a".into(),
                    rdf_gz_url: "a".into(),
                },
            },
        ),
        (
            "b".to_string(),
            CachedExportResponse {
                inserted_at: Instant::now(),
                value: ExportUrlResponse {
                    query: "b".into(),
                    csv_url: "b".into(),
                    json_url: "b".into(),
                    rdf_url: "b".into(),
                    csv_gz_url: "b".into(),
                    json_gz_url: "b".into(),
                    rdf_gz_url: "b".into(),
                },
            },
        ),
    ]);

    prune_cache(&mut cache, Duration::from_mins(1), 1, |entry| {
        entry.inserted_at
    });
    assert!(cache.contains_key("b"));
    assert!(!cache.contains_key("a"));
}

#[tokio::test]
async fn health_route_returns_ok() {
    let config = test_config();
    let app = build_router(config.max_body_bytes, &config, AppState::new(&config));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("health response");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn metrics_route_returns_runtime_counters() {
    let config = test_config();
    let app = build_router(config.max_body_bytes, &config, AppState::new(&config));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("metrics response");

    assert_eq!(response.status(), StatusCode::OK);
    assert!(content_type_header(&response).starts_with("text/plain"));

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("metrics body bytes");
    let body = String::from_utf8(body.to_vec()).expect("utf8 metrics body");
    assert!(body.contains("lotus_api_uptime_seconds"));
    assert!(body.contains("lotus_api_search_cache_hits"));
}

#[tokio::test]
async fn secure_headers_are_applied() {
    let config = test_config();
    let app = build_router(config.max_body_bytes, &config, AppState::new(&config));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("health response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::X_CONTENT_TYPE_OPTIONS)
            .and_then(|value| value.to_str().ok()),
        Some("nosniff")
    );
    assert_eq!(
        response
            .headers()
            .get(header::REFERRER_POLICY)
            .and_then(|value| value.to_str().ok()),
        Some("no-referrer")
    );
}

#[tokio::test]
async fn unknown_route_returns_not_found() {
    let config = test_config();
    let app = build_router(config.max_body_bytes, &config, AppState::new(&config));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/does-not-exist")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("not found response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn client_routes_return_200_and_unknown_pages_return_404() {
    let public_dir = tempfile::tempdir().expect("public directory");
    fs::write(public_dir.path().join("index.html"), "index").expect("index file");
    fs::write(public_dir.path().join("404.html"), "not found").expect("404 file");
    for route in ["search", "curation", "draw"] {
        let route_dir = public_dir.path().join(route);
        fs::create_dir_all(&route_dir).expect("route directory");
        fs::write(route_dir.join("index.html"), "index").expect("route index file");
    }
    let mut config = test_config();
    config.public_dir = Some(public_dir.path().to_path_buf());
    let app = build_router(config.max_body_bytes, &config, AppState::new(&config));

    for uri in [
        "/search?lang=fr",
        "/curation?lang=fr",
        "/draw?dark_mode=true",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("client response");
        assert_eq!(response.status(), StatusCode::OK, "route: {uri}");
        assert_eq!(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("route body"),
            "index"
        );
    }

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/missing")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("missing page response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("404 body"),
        "not found"
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/does-not-exist")
                .body(Body::empty())
                .expect("api request"),
        )
        .await
        .expect("api response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn export_file_rejects_unsupported_format() {
    let config = test_config();
    let app = build_router(config.max_body_bytes, &config, AppState::new(&config));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/export-file/some-key/ttl")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("unsupported format response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(content_type_header(&response).starts_with("application/json"));

    let json = body_json(response).await;
    assert!(
        json.get("error")
            .and_then(serde_json::Value::as_str)
            .is_some()
    );
}

#[tokio::test]
async fn search_rejects_malformed_json_payload() {
    let config = test_config();
    let app = build_router(config.max_body_bytes, &config, AppState::new(&config));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/search")
                .header("content-type", "application/json")
                .body(Body::from("{not-json"))
                .expect("request"),
        )
        .await
        .expect("malformed-json response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn search_rejects_semantically_empty_payload() {
    let config = test_config();
    let app = build_router(config.max_body_bytes, &config, AppState::new(&config));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/search")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .expect("request"),
        )
        .await
        .expect("empty search payload response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(content_type_header(&response).starts_with("application/json"));

    let json = body_json(response).await;
    assert!(
        json.get("error")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|msg| msg.contains("taxon") || msg.contains("smiles"))
    );
}

#[tokio::test]
async fn export_url_rejects_semantically_empty_payload() {
    let config = test_config();
    let app = build_router(config.max_body_bytes, &config, AppState::new(&config));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/export-url")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .expect("request"),
        )
        .await
        .expect("empty export payload response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(content_type_header(&response).starts_with("application/json"));

    let json = body_json(response).await;
    assert!(
        json.get("error")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|msg| msg.contains("taxon") || msg.contains("smiles"))
    );
}

#[tokio::test]
async fn openapi_json_endpoint_serves_core_paths() {
    let config = test_config();
    let app = build_router(config.max_body_bytes, &config, AppState::new(&config));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("openapi response");
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("openapi body bytes");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("openapi json");

    assert!(json["paths"].get("/health").is_some());
    assert!(json["paths"].get("/metrics").is_some());
    assert!(json["paths"].get("/v1/search").is_some());
    assert!(json["paths"].get("/v1/export-url").is_some());
    assert!(
        json["paths"]
            .get("/v1/export-file/{cache_key}/{format}")
            .is_some()
    );
}

#[test]
fn openapi_contains_core_paths() {
    let doc = ApiDoc::openapi();
    assert!(doc.paths.paths.contains_key("/health"));
    assert!(doc.paths.paths.contains_key("/metrics"));
    assert!(doc.paths.paths.contains_key("/v1/search"));
    assert!(doc.paths.paths.contains_key("/v1/export-url"));
    assert!(
        doc.paths
            .paths
            .contains_key("/v1/export-file/{cache_key}/{format}")
    );
}

#[test]
fn openapi_contains_error_and_search_schemas() {
    let doc = ApiDoc::openapi();
    let components = doc.components.expect("openapi components");

    assert!(components.schemas.contains_key("ErrorResponse"));
    assert!(components.schemas.contains_key("SearchRequest"));
    assert!(components.schemas.contains_key("SearchResponse"));
    assert!(components.schemas.contains_key("ExportUrlResponse"));
}

#[tokio::test]
async fn export_file_rejects_unknown_cache_key() {
    let config = test_config();
    let app = build_router(config.max_body_bytes, &config, AppState::new(&config));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/export-file/missing/csv")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("unknown key response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(content_type_header(&response).starts_with("application/json"));

    let json = body_json(response).await;
    assert!(
        json.get("error")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|msg| msg.contains("expired") || msg.contains("unknown"))
    );
}

// ── sanitize_download_filename ────────────────────────────────────────────────

#[test]
fn sanitize_filename_replaces_slash_with_underscore() {
    // '/' and '\' are replaced with '_'; leading dots from trim_matches get removed too.
    // "../../etc/passwd" → ".._.._etc_passwd" → trim leading dots → "_.._etc_passwd"
    assert_eq!(
        export::sanitize_download_filename("../../etc/passwd"),
        "_.._etc_passwd"
    );
    assert_eq!(
        export::sanitize_download_filename(r"C:\Windows\system32"),
        "C:_Windows_system32"
    );
}

#[test]
fn sanitize_filename_removes_control_chars() {
    let input = "file\x00name\x1f.csv";
    let result = export::sanitize_download_filename(input);
    assert!(!result.contains('\x00'));
    assert!(!result.contains('\x1f'));
    assert!(result.contains("filename"));
}

#[test]
fn sanitize_filename_trims_leading_trailing_dots() {
    assert_eq!(export::sanitize_download_filename("...hidden"), "hidden");
    assert_eq!(export::sanitize_download_filename("file..."), "file");
}

#[test]
fn sanitize_filename_preserves_normal_names() {
    assert_eq!(
        export::sanitize_download_filename("export_2024-01-15.csv"),
        "export_2024-01-15.csv"
    );
}

#[test]
fn sanitize_filename_empty_or_whitespace_returns_empty() {
    assert_eq!(export::sanitize_download_filename(""), "");
    assert_eq!(export::sanitize_download_filename("   "), "");
}

// ── element-range validation ──────────────────────────────────────────────────

#[test]
fn apply_request_rejects_inverted_element_ranges() {
    let req = SearchRequest {
        taxon: Some("*".to_string()),
        structure: None,
        structure_search: None,
        structure_threshold: None,
        mass_min: None,
        mass_max: None,
        year_min: None,
        year_max: None,
        formula_exact: None,
        c_min: Some(50),
        c_max: Some(10), // max < min — should be rejected
        h_min: None,
        h_max: None,
        n_min: None,
        n_max: None,
        o_min: None,
        o_max: None,
        p_min: None,
        p_max: None,
        s_min: None,
        s_max: None,
        f_state: None,
        cl_state: None,
        br_state: None,
        i_state: None,
        limit: None,
        include_counts: None,
    };
    assert!(apply_request(&req).is_err());
}

// ── smiles_threshold validation ────────────────────────────────────────────────

#[test]
fn apply_request_clamps_similarity_threshold() {
    fn make_req(threshold: f64) -> SearchRequest {
        SearchRequest {
            taxon: Some("*".to_string()),
            structure: Some("c1ccccc1".to_string()),
            structure_search: None,
            structure_threshold: Some(threshold),
            mass_min: None,
            mass_max: None,
            year_min: None,
            year_max: None,
            formula_exact: None,
            c_min: None,
            c_max: None,
            h_min: None,
            h_max: None,
            n_min: None,
            n_max: None,
            o_min: None,
            o_max: None,
            p_min: None,
            p_max: None,
            s_min: None,
            s_max: None,
            f_state: None,
            cl_state: None,
            br_state: None,
            i_state: None,
            limit: None,
            include_counts: None,
        }
    }

    // threshold = 0 must be rejected
    assert!(
        apply_request(&make_req(0.0)).is_err(),
        "threshold 0 should be rejected"
    );

    // threshold = 2.0 is clamped to 1.0
    let c = apply_request(&make_req(2.0)).expect("over-one threshold");
    assert!(
        c.structure_threshold <= 1.0,
        "threshold should be clamped to maximum 1.0"
    );

    // threshold = 0.5 is within range
    let c = apply_request(&make_req(0.5)).expect("valid threshold");
    assert!((c.structure_threshold - 0.5).abs() < f64::EPSILON);
}

// ── lotus-api .expect() audit (#8) ─────────────────────────────────────────────
//
// The four production `.expect("... inflight mutex")` calls in `state.rs` were
// converted to `Result<(…, ApiError)>` with `.map_err(|_| ApiError::upstream(...))`.
// These tests verify that a poisoned mutex now yields a typed 500 error instead
// of a panic.

/// Helper: poison a `Mutex` by spawning a thread that locks it and panics.
fn poison_inflight_mutex<T: std::marker::Send + 'static>(
    mutex: &std::sync::Arc<std::sync::Mutex<T>>,
) {
    let mutex_clone = std::sync::Arc::clone(mutex);
    std::thread::spawn(move || {
        let _guard = mutex_clone.lock().unwrap();
        panic!("intentional poison for test");
    })
    .join()
    .expect_err("poisoning thread should panic");
}

#[test]
fn search_inflight_cell_returns_error_on_poisoned_mutex() {
    let config = test_config();
    let state = AppState::new(&config);
    poison_inflight_mutex(&state.search_inflight);

    let result = search_inflight_cell(&state, "test-key");
    let err = result.expect_err("poisoned mutex should return Err, not panic");
    assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(err.message.contains("inflight"));
}

#[test]
fn export_inflight_cell_returns_error_on_poisoned_mutex() {
    let config = test_config();
    let state = AppState::new(&config);
    poison_inflight_mutex(&state.export_inflight);

    let result = export_inflight_cell(&state, "test-key");
    let err = result.expect_err("poisoned mutex should return Err, not panic");
    assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(err.message.contains("inflight"));
}

#[tokio::test]
async fn search_handler_returns_500_on_poisoned_inflight_mutex() {
    let config = test_config();
    let state = AppState::new(&config);
    poison_inflight_mutex(&state.search_inflight);

    let app = build_router(config.max_body_bytes, &config, state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/search")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"taxon":"*"}"#))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(content_type_header(&response).starts_with("application/json"));
    let json = body_json(response).await;
    let msg = json["error"].as_str().expect("error message");
    assert!(msg.contains("inflight"));
}

#[tokio::test]
async fn export_handler_returns_500_on_poisoned_inflight_mutex() {
    let config = test_config();
    let state = AppState::new(&config);
    poison_inflight_mutex(&state.export_inflight);

    let app = build_router(config.max_body_bytes, &config, state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/export-url")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"taxon":"*"}"#))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(content_type_header(&response).starts_with("application/json"));
    let json = body_json(response).await;
    let msg = json["error"].as_str().expect("error message");
    assert!(msg.contains("inflight"));
}
