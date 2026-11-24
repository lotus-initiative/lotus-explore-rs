// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Authenticated HTTP client, retries, and response decoding for Zenodo.

use std::fmt;
use std::time::Duration;

use anyhow::{Context, bail};
use reqwest::Method;
use reqwest::header::{ACCEPT, AUTHORIZATION};

use super::record::Deposition;

pub(super) const PRODUCTION_API_URL: &str = "https://zenodo.org/api";
pub(super) const SANDBOX_API_URL: &str = "https://sandbox.zenodo.org/api";
const USER_AGENT: &str = concat!("lotus-cli/", env!("CARGO_PKG_VERSION"));
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(500);
const DEFAULT_MAX_POLLS: usize = 24;
pub(super) const MAX_TRANSIENT_ATTEMPTS: usize = 3;

/// Authenticated HTTP client for Zenodo's deposition REST API.
pub struct ZenodoClient {
    http: reqwest::Client,
    pub(super) base_url: String,
    token: String,
    pub(super) poll_interval: Duration,
    pub(super) max_polls: usize,
    retry_delay: Duration,
}

impl fmt::Debug for ZenodoClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ZenodoClient")
            .field("base_url", &self.base_url)
            .field("token", &"<redacted>")
            .field("poll_interval", &self.poll_interval)
            .field("max_polls", &self.max_polls)
            .finish_non_exhaustive()
    }
}

impl ZenodoClient {
    /// Builds a client from `ZENODO_SANDBOX_TOKEN` (preferred) or `ZENODO_TOKEN`.
    ///
    /// # Errors
    /// Returns an error if neither environment variable holds a non-empty token
    /// or if the HTTP client cannot be constructed.
    pub fn from_env_or_sandbox() -> anyhow::Result<(Self, &'static str)> {
        Self::from_env_with(|name| std::env::var(name))
    }

    pub(super) fn from_env_with(
        mut get_env: impl FnMut(&str) -> Result<String, std::env::VarError>,
    ) -> anyhow::Result<(Self, &'static str)> {
        if let Some(token) = read_non_empty_env("ZENODO_SANDBOX_TOKEN", &mut get_env)? {
            let client = Self::with_base_url(SANDBOX_API_URL, token)?;
            return Ok((client, "Zenodo sandbox (ZENODO_SANDBOX_TOKEN)"));
        }
        let token = read_non_empty_env("ZENODO_TOKEN", &mut get_env)?
            .context("read ZENODO_TOKEN: environment variable is not set or empty")?;
        let client = Self::with_base_url(PRODUCTION_API_URL, token)?;
        Ok((client, "Zenodo production (ZENODO_TOKEN)"))
    }

    /// Builds a client targeting an explicit Zenodo API base URL (e.g. `/api`).
    ///
    /// # Errors
    /// Returns an error if `token` is blank, `base_url` is invalid, or the
    /// underlying HTTP client fails to initialize.
    pub fn with_base_url(
        base_url: impl Into<String>,
        token: impl Into<String>,
    ) -> anyhow::Result<Self> {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
            bail!("Zenodo base URL must start with http:// or https://");
        }
        let token = token.into();
        if token.trim().is_empty() {
            bail!("Zenodo API token must not be empty");
        }
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
            .timeout(DEFAULT_TIMEOUT)
            .build()
            .context("build Zenodo HTTP client")?;
        Ok(Self {
            http,
            base_url,
            token,
            poll_interval: DEFAULT_POLL_INTERVAL,
            max_polls: DEFAULT_MAX_POLLS,
            retry_delay: Duration::from_millis(250),
        })
    }

    #[cfg(test)]
    pub(super) const fn with_fast_timings(mut self) -> Self {
        self.poll_interval = Duration::from_millis(1);
        self.max_polls = 10;
        self.retry_delay = Duration::from_millis(1);
        self
    }

    pub(super) fn request(&self, method: Method, url: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(ACCEPT, "application/json")
    }

    pub(super) async fn send_with_retries<T, F, Fut>(
        &self,
        mut build_request: impl FnMut() -> reqwest::RequestBuilder,
        op: &'static str,
        parse: F,
    ) -> anyhow::Result<T>
    where
        F: Fn(reqwest::Response, &'static str) -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<T>>,
    {
        let mut delay = self.retry_delay;
        for attempt in 1..=MAX_TRANSIENT_ATTEMPTS {
            match build_request().send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if is_transient_status(status.as_u16()) && attempt < MAX_TRANSIENT_ATTEMPTS {
                        tokio::time::sleep(delay).await;
                        delay = delay.saturating_mul(2);
                        continue;
                    }
                    return parse(resp, op).await;
                }
                Err(err) => {
                    if attempt == MAX_TRANSIENT_ATTEMPTS {
                        return Err(err).with_context(|| format!("{op} failed"));
                    }
                    tokio::time::sleep(delay).await;
                    delay = delay.saturating_mul(2);
                }
            }
        }
        bail!("{op} exhausted retry attempts");
    }
}

const fn is_transient_status(code: u16) -> bool {
    matches!(code, 429 | 500..=599)
}

pub(super) fn read_non_empty_env(
    name: &str,
    get_env: &mut impl FnMut(&str) -> Result<String, std::env::VarError>,
) -> anyhow::Result<Option<String>> {
    match get_env(name) {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        Ok(_) | Err(std::env::VarError::NotPresent) => Ok(None),
        Err(err) => bail!("cannot read {name}: {err}"),
    }
}

pub(super) async fn ensure_success(
    response: reqwest::Response,
    op: &str,
) -> anyhow::Result<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = response.text().await.unwrap_or_default();
    let trimmed = body.trim();
    if trimmed.is_empty() {
        bail!("{op}: HTTP {status}");
    }
    bail!("{op}: HTTP {status}: {trimmed}");
}

pub(super) async fn parse_json_response<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
    op: &str,
) -> anyhow::Result<T> {
    let response = ensure_success(response, op).await?;
    let bytes = response
        .bytes()
        .await
        .with_context(|| format!("{op}: read response body"))?;
    serde_json::from_slice(&bytes).with_context(|| format!("{op}: decode Zenodo JSON"))
}

pub(super) async fn parse_action_deposition<F, Fut>(
    response: reqwest::Response,
    fallback_get: F,
) -> anyhow::Result<Deposition>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<Deposition>>,
{
    let response = ensure_success(response, "Zenodo action").await?;
    let bytes = response.bytes().await.context("read Zenodo action body")?;
    if bytes.is_empty() {
        return fallback_get().await;
    }
    match serde_json::from_slice::<Deposition>(&bytes) {
        Ok(deposition) => Ok(deposition),
        Err(_) => fallback_get().await,
    }
}
