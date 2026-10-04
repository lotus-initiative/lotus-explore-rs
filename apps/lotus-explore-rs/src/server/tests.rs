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

use crate::export;
use crate::server::{
    ApiDoc, build_router,
    config::AppConfig,
    query_logic::{apply_request, build_execution_query},
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
        reference: None,
        taxon_accepted_synonyms: None,
        taxon_basionyms: None,
        taxon_protonyms: None,
        taxon_replacements: None,
        smiles: None,
        smiles_search_type: None,
        similarity_threshold: None,
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

/// A molfile's newlines survive normalisation.
///
/// This used to assert on a server-local copy of the normaliser, which was a
/// verbatim duplicate of `lotus_search::normalize_structure`. The duplicate is
/// gone, so this now pins the one implementation both front ends use -- which is
/// the point: a molfile trimmed to one line is a molfile the structure service
/// cannot read.
#[test]
fn normalized_structure_preserves_multiline_molfile() {
    let molfile = "\n  Mrv\n\n  0  0  0  0  0  0            999 V3000\nM  END\n";
    let normalized = lotus_search::normalize_structure(molfile);
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

#[test]
fn apply_request_rejects_inverted_element_ranges() {
    let req = SearchRequest {
        taxon: Some("*".to_string()),
        reference: None,
        taxon_accepted_synonyms: None,
        taxon_basionyms: None,
        taxon_protonyms: None,
        taxon_replacements: None,
        smiles: None,
        smiles_search_type: None,
        similarity_threshold: None,
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

#[test]
fn apply_request_clamps_similarity_threshold() {
    fn make_req(threshold: f64) -> SearchRequest {
        SearchRequest {
            taxon: Some("*".to_string()),
            reference: None,
            taxon_accepted_synonyms: None,
            taxon_basionyms: None,
            taxon_protonyms: None,
            taxon_replacements: None,
            smiles: Some("c1ccccc1".to_string()),
            smiles_search_type: None,
            similarity_threshold: Some(threshold),
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

/// The API and the browser must build the same query for the same request.
///
/// The server used to carry its own copy of the dispatch in
/// `query_logic::build_execution_query`, and the two disagreed about the empty
/// taxon box: the API ran `all_compounds_query`, which requires `P703`, while the
/// browser ran `all_compounds_including_untaxonomised_query`, which does not. The
/// cause was that the server resolves a wildcard to `Some("*")` where the library
/// resolves it to `None` -- so the server had to treat `None` as a wildcard, and
/// `None` is also what a blank box produces.
///
/// One implementation now, so this cannot drift again by duplication. It is
/// asserted rather than assumed because "the two front ends agree" is a property
/// nothing else in the suite can see: each front end's own tests pass either way.
#[test]
fn the_api_and_the_browser_build_the_same_query() {
    let cases: [(&str, Option<&str>); 4] = [
        // (criteria taxon, the QID the server's resolver hands the builder)
        ("", None),
        ("*", Some("*")),
        ("Gentiana lutea", Some("Q16521")),
        ("Q16521", Some("Q16521")),
    ];

    for (input, resolved) in cases {
        let criteria = lotus_model::SearchCriteria {
            taxon: input.to_string(),
            ..lotus_model::SearchCriteria::up_to_year(crate::clock::current_year())
        };

        let via_server = build_execution_query(&criteria, resolved);
        let request = lotus_search::SearchRequest::new(criteria, crate::clock::current_year());
        let via_library = lotus_search::build_execution_query(&request, resolved);

        assert_eq!(
            via_server, via_library,
            "taxon {input:?} must build the same query on both front ends"
        );
    }
}

/// A blank taxon box includes the compounds nobody has tied to an organism.
///
/// This is the behaviour the divergence hid. The occurrence is `OPTIONAL`, so a
/// compound with no organism comes back with empty cells; requiring `P703` would
/// answer the narrower question -- "what has been reported, and where" -- without
/// saying so, which is the bug commit 4ead47a describes.
#[test]
fn a_blank_taxon_box_does_not_require_an_occurrence() {
    let criteria = lotus_model::SearchCriteria {
        taxon: String::new(),
        ..lotus_model::SearchCriteria::up_to_year(crate::clock::current_year())
    };

    let query = build_execution_query(&criteria, None);

    // The occurrence sits inside one OPTIONAL block rather than being a plain
    // triple, which is what makes the untaxonomised compounds reachable.
    let p703 = query.find("?c p:P703").expect("P703 is bound");
    let head = &query[..p703];
    let optional = head.rfind("OPTIONAL").is_some_and(|at| {
        query[at + "OPTIONAL".len()..p703]
            .chars()
            .all(|c| c.is_whitespace() || c == '{')
    });

    assert!(
        optional,
        "an empty taxon box must reach the compounds with no organism:\n{query}"
    );
}

/// A wildcard still requires an occurrence, on the API as in the browser.
///
/// The other half of the pair, and the reason the empty-box case was easy to get
/// wrong: `*` is the explicit request for what has been reported.
#[test]
fn a_wildcard_taxon_still_requires_an_occurrence() {
    let criteria = lotus_model::SearchCriteria {
        taxon: "*".to_string(),
        ..lotus_model::SearchCriteria::up_to_year(crate::clock::current_year())
    };

    let query = build_execution_query(&criteria, Some("*"));
    let p703 = query.find("?c p:P703").expect("P703 is bound");
    let head = &query[..p703];
    let optional = head.rfind("OPTIONAL").is_some_and(|at| {
        query[at + "OPTIONAL".len()..p703]
            .chars()
            .all(|c| c.is_whitespace() || c == '{')
    });

    assert!(
        !optional,
        "`*` asks for what has been reported, so it keeps requiring P703:\n{query}"
    );
}
