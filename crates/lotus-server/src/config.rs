// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Server configuration management from CLI arguments and environment variables.

use std::{net::SocketAddr, time::Duration};

use axum::http::HeaderValue;
use clap::Parser;
use thiserror::Error;
use tower_http::cors::{Any, CorsLayer};

/// Typed errors raised while loading or validating server configuration.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConfigError {
    /// Port value failed `u16` parsing.
    #[error("{name} must be a valid u16: {detail}")]
    InvalidPort {
        /// Environment variable / flag name (`PORT`).
        name: &'static str,
        /// Parse error description.
        detail: String,
    },
    /// Numeric setting failed non-negative integer parsing.
    #[error("{name} must be a valid non-negative integer: {detail}")]
    InvalidUsize {
        /// Environment variable / flag name.
        name: &'static str,
        /// Parse error description.
        detail: String,
    },
    /// `APP_ENV=production` requires an explicit CORS origin allowlist.
    #[error("APP_ENV=production requires CORS_ALLOWED_ORIGINS to be configured")]
    MissingProductionCors,
    /// A configured CORS origin lacked an `http://` or `https://` scheme.
    #[error("CORS_ALLOWED_ORIGINS entry '{origin}' must start with http:// or https://")]
    InvalidCorsScheme {
        /// Offending origin token.
        origin: String,
    },
    /// A configured CORS origin was not a valid HTTP header value.
    #[error("CORS_ALLOWED_ORIGINS contains invalid origin '{origin}'")]
    InvalidCorsHeader {
        /// Offending origin token.
        origin: String,
    },
    /// `HOST:PORT` could not be parsed as a [`SocketAddr`].
    #[error("invalid bind address '{addr}': {detail}")]
    InvalidBindAddr {
        /// Combined `host:port` string.
        addr: String,
        /// Parse error description.
        detail: String,
    },
}

/// Validated runtime configuration for the HTTP API server.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) default_limit: usize,
    pub(crate) request_timeout: Duration,
    pub(crate) max_concurrency: usize,
    /// How many queries may be in flight to `QLever` at once.
    pub(crate) upstream_concurrency: usize,
    /// How long a search waits for an upstream permit before being shed.
    pub(crate) upstream_queue_wait: Duration,
    pub(crate) max_body_bytes: usize,
    pub(crate) cors_allowed_origins: Option<Vec<HeaderValue>>,
    pub(crate) public_dir: Option<std::path::PathBuf>,
}

/// Command-line and environment configuration for `lotus-server`.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "lotus-server",
    version,
    about = "OpenAPI service for LOTUS explorer search and export workflows",
    long_about = None,
)]
struct Cli {
    #[arg(long, env = "HOST", default_value = "127.0.0.1")]
    host: String,
    #[arg(long, env = "PORT", default_value = "8787")]
    port: String,
    #[arg(long, env = "DEFAULT_LIMIT", default_value = "500")]
    default_limit: String,
    #[arg(long, env = "REQUEST_TIMEOUT_MS", default_value = "45000")]
    request_timeout_ms: String,
    #[arg(long, env = "MAX_CONCURRENCY", default_value = "256")]
    max_concurrency: String,
    /// Queries in flight to `QLever` at once.
    #[arg(long, env = "UPSTREAM_CONCURRENCY", default_value = "4")]
    upstream_concurrency: String,
    #[arg(long, env = "MAX_BODY_BYTES", default_value = "1048576")]
    max_body_bytes: String,
    #[arg(long, env = "APP_ENV", default_value = "development")]
    app_env: String,
    #[arg(long, env = "CORS_ALLOWED_ORIGINS")]
    cors_allowed_origins: Option<String>,
    #[arg(long, env = "PUBLIC_DIR")]
    public_dir: Option<std::path::PathBuf>,
}

impl Cli {
    fn get(&self, name: &str) -> Option<String> {
        match name {
            "HOST" => Some(self.host.clone()),
            "PORT" => Some(self.port.clone()),
            "DEFAULT_LIMIT" => Some(self.default_limit.clone()),
            "REQUEST_TIMEOUT_MS" => Some(self.request_timeout_ms.clone()),
            "MAX_CONCURRENCY" => Some(self.max_concurrency.clone()),
            "UPSTREAM_CONCURRENCY" => Some(self.upstream_concurrency.clone()),
            "MAX_BODY_BYTES" => Some(self.max_body_bytes.clone()),
            "APP_ENV" => Some(self.app_env.clone()),
            "CORS_ALLOWED_ORIGINS" => self.cors_allowed_origins.clone(),
            "PUBLIC_DIR" => self
                .public_dir
                .as_ref()
                .and_then(|p| p.to_str())
                .map(String::from),
            _ => None,
        }
    }
}

impl AppConfig {
    /// Build configuration from the process command line and environment.
    pub(crate) fn from_env() -> Result<Self, ConfigError> {
        let cli = Cli::parse();
        Self::from_provider(|name| cli.get(name))
    }

    /// Build configuration from a caller-supplied key lookup (used in tests).
    ///
    /// # Errors
    /// Returns [`ConfigError`] when any numeric field, CORS origin, or
    /// production allowlist requirement fails validation.
    pub fn from_provider<F>(mut get: F) -> Result<Self, ConfigError>
    where
        F: FnMut(&str) -> Option<String>,
    {
        let host = get("HOST")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "127.0.0.1".into());

        let port = parse_u16_env(get("PORT"), "PORT", 8787)?;
        let default_limit = parse_usize_env(get("DEFAULT_LIMIT"), "DEFAULT_LIMIT", 500)?
            .clamp(1, lotus_explore_core::table_budget::API_MAX_ROWS);
        let request_timeout_ms =
            parse_usize_env(get("REQUEST_TIMEOUT_MS"), "REQUEST_TIMEOUT_MS", 45_000)?
                .clamp(1_000, 300_000);
        let max_concurrency =
            parse_usize_env(get("MAX_CONCURRENCY"), "MAX_CONCURRENCY", 256)?.clamp(8, 4_096);
        let upstream_concurrency =
            parse_usize_env(get("UPSTREAM_CONCURRENCY"), "UPSTREAM_CONCURRENCY", 4)?.clamp(1, 64);
        let max_body_bytes = parse_usize_env(get("MAX_BODY_BYTES"), "MAX_BODY_BYTES", 1_048_576)?
            .clamp(4 * 1024, 16 * 1024 * 1024);

        let app_env = get("APP_ENV")
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "development".into());

        let cors_allowed_origins = parse_allowed_origins(get("CORS_ALLOWED_ORIGINS"))?;
        if app_env == "production" && cors_allowed_origins.is_none() {
            return Err(ConfigError::MissingProductionCors);
        }

        let public_dir = get("PUBLIC_DIR").map(std::path::PathBuf::from);

        Ok(Self {
            host,
            port,
            default_limit,
            request_timeout: Duration::from_millis(request_timeout_ms as u64),
            max_concurrency,
            upstream_concurrency,
            upstream_queue_wait: Duration::from_millis(
                (request_timeout_ms / 4).clamp(250, 5_000) as u64
            ),
            max_body_bytes,
            cors_allowed_origins,
            public_dir,
        })
    }

    pub(crate) fn bind_addr(&self) -> Result<SocketAddr, ConfigError> {
        let addr = format!("{}:{}", self.host, self.port);
        addr.parse::<SocketAddr>()
            .map_err(|e| ConfigError::InvalidBindAddr {
                addr,
                detail: e.to_string(),
            })
    }
}

fn parse_u16_env(
    value: Option<String>,
    name: &'static str,
    default_value: u16,
) -> Result<u16, ConfigError> {
    value.map_or(Ok(default_value), |raw| {
        raw.trim()
            .parse::<u16>()
            .map_err(|e| ConfigError::InvalidPort {
                name,
                detail: e.to_string(),
            })
    })
}

fn parse_usize_env(
    value: Option<String>,
    name: &'static str,
    default_value: usize,
) -> Result<usize, ConfigError> {
    value.map_or(Ok(default_value), |raw| {
        raw.trim()
            .parse::<usize>()
            .map_err(|e| ConfigError::InvalidUsize {
                name,
                detail: e.to_string(),
            })
    })
}

fn parse_allowed_origins(value: Option<String>) -> Result<Option<Vec<HeaderValue>>, ConfigError> {
    let Some(raw) = value else {
        return Ok(None);
    };

    let mut origins = Vec::new();
    for origin in raw
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        if !origin.starts_with("http://") && !origin.starts_with("https://") {
            return Err(ConfigError::InvalidCorsScheme {
                origin: origin.to_string(),
            });
        }
        let header = HeaderValue::from_str(origin).map_err(|_| ConfigError::InvalidCorsHeader {
            origin: origin.to_string(),
        })?;
        origins.push(header);
    }

    if origins.is_empty() {
        Ok(None)
    } else {
        Ok(Some(origins))
    }
}

/// Build the CORS middleware layer described by `config`.
pub(crate) fn build_cors_layer(config: &AppConfig) -> CorsLayer {
    let layer = CorsLayer::new().allow_methods(Any).allow_headers(Any);
    match &config.cors_allowed_origins {
        Some(origins) => layer.allow_origin(origins.clone()),
        None => layer.allow_origin(Any),
    }
}

#[cfg(test)]
#[path = "config/tests.rs"]
mod tests;
