// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
// This crate exposes the router/types cleanly for its integration & unit tests
// and the `lotus-server` binary; some internal test helpers live in `crate` submodules.

//! Native HTTP API and static SPA host for the LOTUS Explorer.

mod cache_key;
mod clock;
pub mod config;
pub mod errors;
mod export_urls;
pub mod handlers;
mod logging;
mod query_logic;
mod services;
pub mod state;
pub mod types;

#[cfg(test)]
mod characterization_tests;
#[cfg(test)]
mod tests;

pub use cache_key::{build_export_cache_key, build_search_cache_key};

use axum::{
    Json, Router,
    body::Body,
    extract::DefaultBodyLimit,
    http::{HeaderName, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::Response,
    routing::{any, get, get_service, post},
};
use tower_http::{
    compression::CompressionLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    services::{ServeDir, ServeFile},
};
use utoipa::OpenApi;

use crate::{
    config::{AppConfig, build_cors_layer},
    errors::ErrorResponse,
    handlers::{export_file, export_urls, health, metrics, search},
    state::AppState,
    types::{
        ApiElementState, ApiSmilesSearchType, ExportUrlResponse, HealthResponse, RowDto,
        SearchRequest, SearchResponse, SearchStats,
    },
};

const X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::health,
        handlers::metrics,
        handlers::search,
        handlers::export_urls,
        handlers::export_file
    ),
    components(
        schemas(
            HealthResponse,
            SearchRequest,
            SearchResponse,
            SearchStats,
            RowDto,
            ExportUrlResponse,
            ErrorResponse,
            ApiSmilesSearchType,
            ApiElementState
        )
    ),
    tags((name = "lotus-explore-rs", description = "LOTUS explorer programmatic API"))
)]
pub(crate) struct ApiDoc;

async fn api_not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

/// Build the Axum [`Router`] with middleware, `/v1/*` routes, and optional static SPA fallback.
pub fn build_router(max_body_bytes: usize, config: &AppConfig, state: AppState) -> Router {
    let mut router = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route("/openapi.json", get(openapi_json))
        .route("/v1/search", post(search))
        .route("/v1/export-url", post(export_urls))
        .route("/v1/export-file/{cache_key}/{format}", get(export_file))
        .route("/v1/{*path}", any(api_not_found))
        .with_state(state)
        .layer(DefaultBodyLimit::max(max_body_bytes))
        .layer(middleware::from_fn(log_request))
        .layer(PropagateRequestIdLayer::new(X_REQUEST_ID));

    if let Some(public_dir) = &config.public_dir {
        let route_index = |route: &str| {
            let candidate = public_dir.join(route).join("index.html");
            let file = if candidate.exists() {
                candidate
            } else {
                public_dir.join("index.html")
            };
            get_service(ServeFile::new(file).precompressed_br())
        };
        router = router
            .route("/search", route_index("search"))
            .route("/network", route_index("network"))
            .route("/curation", route_index("curation"))
            .route("/draw", route_index("draw"))
            .route("/faq", route_index("faq"))
            .fallback_service(
                ServeDir::new(public_dir.clone())
                    .precompressed_br()
                    .not_found_service(ServeFile::new(public_dir.join("404.html"))),
            );
    }

    router
        .layer(SetRequestIdLayer::new(X_REQUEST_ID, MakeRequestUuid))
        .layer(CompressionLayer::new())
        .layer(build_cors_layer(config))
        .layer(middleware::from_fn(add_security_headers))
}

async fn log_request(req: axum::http::Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let started = std::time::Instant::now();
    let response = next.run(req).await;
    let status = response.status().as_u16();
    let elapsed_ms = started.elapsed().as_millis();
    if response.status().is_server_error() {
        log::warn!(
            "event=http method={method} path={path} status={status} elapsed_ms={elapsed_ms}"
        );
    } else {
        log::info!(
            "event=http method={method} path={path} status={status} elapsed_ms={elapsed_ms}"
        );
    }
    response
}

async fn add_security_headers(req: axum::http::Request<Body>, next: Next) -> Response {
    let mut response = next.run(req).await;
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

/// Start the HTTP API server, listening until `SIGINT` or `SIGTERM`.
///
/// # Errors
/// Returns an error when configuration parsing, socket binding, or `axum::serve` fails.
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    logging::init();

    let config = config::AppConfig::from_env()?;
    let state = state::AppState::new(&config);
    let app = build_router(config.max_body_bytes, &config, state);

    let addr = config.bind_addr()?;
    log::info!(
        "lotus-server listening on http://{addr} timeout_ms={} max_concurrency={} max_body_bytes={}",
        config.request_timeout.as_millis(),
        config.max_concurrency,
        config.max_body_bytes,
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            let _ = signal.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => { log::info!("event=shutdown signal=ctrl_c"); },
        () = terminate => { log::info!("event=shutdown signal=terminate"); },
    }
}
