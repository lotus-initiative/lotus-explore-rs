// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Application configuration management from environment variables.

use axum::http::HeaderValue;
use clap::Parser;
use std::{net::SocketAddr, time::Duration};
use tower_http::cors::{Any, CorsLayer};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) default_limit: usize,
    pub(crate) request_timeout: Duration,
    pub(crate) max_concurrency: usize,
    pub(crate) max_body_bytes: usize,
    pub(crate) cors_allowed_origins: Option<Vec<HeaderValue>>,
    pub(crate) public_dir: Option<std::path::PathBuf>,
}

/// Command-line and environment configuration for the in-package `lotus-explore-rs` server.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "lotus-explore-rs",
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
    /// Adapter that lets `AppConfig::from_provider` read the clap-resolved value
    /// for a given env-var name (`HOST`, `PORT`, ...), so the existing
    /// validation path is reused verbatim.
    fn get(&self, name: &str) -> Option<String> {
        match name {
            "HOST" => Some(self.host.clone()),
            "PORT" => Some(self.port.clone()),
            "DEFAULT_LIMIT" => Some(self.default_limit.clone()),
            "REQUEST_TIMEOUT_MS" => Some(self.request_timeout_ms.clone()),
            "MAX_CONCURRENCY" => Some(self.max_concurrency.clone()),
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
    pub(crate) fn from_env() -> Result<Self, String> {
        let cli = Cli::parse();
        Self::from_provider(|name| cli.get(name))
    }

    pub(crate) fn from_provider<F>(mut get: F) -> Result<Self, String>
    where
        F: FnMut(&str) -> Option<String>,
    {
        let host = get("HOST")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "127.0.0.1".into());

        let port = parse_u16_env(get("PORT"), "PORT", 8787)?;
        let default_limit = parse_usize_env(get("DEFAULT_LIMIT"), "DEFAULT_LIMIT", 500)?
            .clamp(1, crate::table_budget::API_MAX_ROWS);
        let request_timeout_ms =
            parse_usize_env(get("REQUEST_TIMEOUT_MS"), "REQUEST_TIMEOUT_MS", 45_000)?
                .clamp(1_000, 300_000);
        let max_concurrency =
            parse_usize_env(get("MAX_CONCURRENCY"), "MAX_CONCURRENCY", 256)?.clamp(8, 4_096);
        let max_body_bytes = parse_usize_env(get("MAX_BODY_BYTES"), "MAX_BODY_BYTES", 1_048_576)?
            .clamp(4 * 1024, 16 * 1024 * 1024);

        let app_env = get("APP_ENV")
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "development".into());

        let cors_allowed_origins = parse_allowed_origins(get("CORS_ALLOWED_ORIGINS"))?;
        if app_env == "production" && cors_allowed_origins.is_none() {
            return Err("APP_ENV=production requires CORS_ALLOWED_ORIGINS to be configured".into());
        }

        let public_dir = get("PUBLIC_DIR").map(std::path::PathBuf::from);

        Ok(Self {
            host,
            port,
            default_limit,
            request_timeout: Duration::from_millis(request_timeout_ms as u64),
            max_concurrency,
            max_body_bytes,
            cors_allowed_origins,
            public_dir,
        })
    }

    pub(crate) fn bind_addr(&self) -> Result<SocketAddr, String> {
        format!("{}:{}", self.host, self.port)
            .parse::<SocketAddr>()
            .map_err(|e| format!("invalid bind address '{}:{}': {e}", self.host, self.port))
    }
}

fn parse_u16_env(value: Option<String>, name: &str, default_value: u16) -> Result<u16, String> {
    value.map_or(Ok(default_value), |raw| {
        raw.trim()
            .parse::<u16>()
            .map_err(|e| format!("{name} must be a valid u16: {e}"))
    })
}

fn parse_usize_env(
    value: Option<String>,
    name: &str,
    default_value: usize,
) -> Result<usize, String> {
    value.map_or(Ok(default_value), |raw| {
        raw.trim()
            .parse::<usize>()
            .map_err(|e| format!("{name} must be a valid non-negative integer: {e}"))
    })
}

fn parse_allowed_origins(value: Option<String>) -> Result<Option<Vec<HeaderValue>>, String> {
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
            return Err(format!(
                "CORS_ALLOWED_ORIGINS entry '{origin}' must start with http:// or https://"
            ));
        }
        let header = HeaderValue::from_str(origin)
            .map_err(|_| format!("CORS_ALLOWED_ORIGINS contains invalid origin '{origin}'"))?;
        origins.push(header);
    }

    if origins.is_empty() {
        Ok(None)
    } else {
        Ok(Some(origins))
    }
}

pub fn build_cors_layer(config: &AppConfig) -> CorsLayer {
    let layer = CorsLayer::new().allow_methods(Any).allow_headers(Any);
    match &config.cors_allowed_origins {
        Some(origins) => layer.allow_origin(origins.clone()),
        None => layer.allow_origin(Any),
    }
}

// ── Tests for the clap CLI layer ─────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::indexing_slicing)]

    use super::*;

    // ── Cli flag resolution (tested via clap's try_parse_from) ───────
    //
    // clap's `#[arg(env = "...")]` attribute guarantees flag > env > default
    // priority. These tests verify flag resolution and defaults directly.
    // Env-var-only override behavior cannot be tested here because the
    // workspace enforces `#![forbid(unsafe_code)]` and
    // `std::env::set_var` / `remove_var` are unsafe in Rust 1.97+. The
    // existing `from_provider` tests in tests.rs cover the env-value parsing
    // layer with mock closures.

    #[test]
    fn cli_port_flag_overrides_default() {
        let cli = Cli::try_parse_from(["lotus-explore-rs", "--port", "1234"]).unwrap();
        assert_eq!(cli.get("PORT"), Some("1234".to_string()));
    }

    #[test]
    fn cli_port_default_when_no_flag() {
        // Assumes PORT is not set in the test environment (typical for CI).
        let cli = Cli::try_parse_from(["lotus-explore-rs"]).unwrap();
        assert_eq!(cli.get("PORT"), Some("8787".to_string()));
    }

    #[test]
    fn cli_host_flag_overrides_default() {
        let cli = Cli::try_parse_from(["lotus-explore-rs", "--host", "0.0.0.0"]).unwrap();
        assert_eq!(cli.get("HOST"), Some("0.0.0.0".to_string()));
    }

    // ── Invalid port values error cleanly through from_provider ───────

    #[test]
    fn from_provider_invalid_port_string_returns_error() {
        let result = AppConfig::from_provider(|name| {
            if name == "PORT" {
                Some("not-a-port".to_string())
            } else {
                None
            }
        });
        let err = result.expect_err("non-numeric port should error");
        assert!(err.contains("PORT"));
    }

    #[test]
    fn from_provider_negative_port_returns_error() {
        let result = AppConfig::from_provider(|name| {
            if name == "PORT" {
                Some("-1".to_string())
            } else {
                None
            }
        });
        let err = result.expect_err("negative port should error");
        assert!(err.contains("PORT"));
    }

    #[test]
    fn from_provider_port_overflow_returns_error() {
        let result = AppConfig::from_provider(|name| {
            if name == "PORT" {
                Some("70000".to_string())
            } else {
                None
            }
        });
        let err = result.expect_err("port > u16::MAX should error");
        assert!(err.contains("PORT"));
    }

    // ── Integration: Cli flag → from_provider ──────────────────────────

    #[test]
    fn flag_port_through_full_flow() {
        let cli = Cli::try_parse_from(["lotus-explore-rs", "--port", "1234"]).unwrap();
        let cfg =
            AppConfig::from_provider(|name| cli.get(name)).expect("flag port 1234 should parse");
        assert_eq!(cfg.port, 1234);
    }

    #[test]
    fn default_port_through_full_flow() {
        // Assumes PORT is not set in the test environment.
        let cli = Cli::try_parse_from(["lotus-explore-rs"]).unwrap();
        let cfg =
            AppConfig::from_provider(|name| cli.get(name)).expect("default port should parse");
        assert_eq!(cfg.port, 8787);
    }
}
