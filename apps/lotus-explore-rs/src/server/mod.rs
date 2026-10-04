// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Native HTTP API for the LOTUS explorer: search and export endpoints.

mod config;
mod errors;
mod handlers;
mod query_logic;
mod services;
mod state;
mod types;

#[cfg(test)]
mod tests;

use axum::{
    Router,
    body::Body,
    extract::DefaultBodyLimit,
    http::{HeaderName, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::Response,
    routing::{any, get, get_service, post},
};
use handlers::{export_file, export_urls, health, metrics, search};
use tower_http::{
    compression::CompressionLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    services::{ServeDir, ServeFile},
    trace::{DefaultMakeSpan, DefaultOnFailure, DefaultOnRequest, DefaultOnResponse, TraceLayer},
};
use tracing::Level;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::server::{
    config::{AppConfig, build_cors_layer},
    errors::ErrorResponse,
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
struct ApiDoc;

async fn api_not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

pub fn build_router(max_body_bytes: usize, config: &AppConfig, state: AppState) -> Router {
    let mut router = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route("/v1/search", post(search))
        .route("/v1/export-url", post(export_urls))
        .route("/v1/export-file/{cache_key}/{format}", get(export_file))
        .route("/v1/{*path}", any(api_not_found))
        .merge(SwaggerUi::new("/docs").url("/openapi.json", ApiDoc::openapi()))
        .with_state(state)
        .layer(DefaultBodyLimit::max(max_body_bytes))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_request(DefaultOnRequest::new().level(Level::DEBUG))
                .on_response(DefaultOnResponse::new().level(Level::INFO))
                .on_failure(DefaultOnFailure::new().level(Level::WARN)),
        )
        .layer(PropagateRequestIdLayer::new(X_REQUEST_ID));

    // Optionally serve the Dioxus WASM build output as static files.
    if let Some(public_dir) = &config.public_dir {
        let route_index = |route: &str| {
            get_service(
                ServeFile::new(public_dir.join(route).join("index.html")).precompressed_br(),
            )
        };
        router = router
            .route("/search", route_index("search"))
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

/// Entry point for the in-package native API server.
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let config = config::AppConfig::from_env().map_err(std::io::Error::other)?;
    let state = state::AppState::new(&config);
    let app = build_router(config.max_body_bytes, &config, state);

    let addr = config.bind_addr().map_err(std::io::Error::other)?;
    log::info!(
        "lotus-explore-rs server listening on http://{addr} timeout_ms={} max_concurrency={} max_body_bytes={}",
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
