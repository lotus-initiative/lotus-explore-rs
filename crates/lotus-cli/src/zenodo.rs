// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Minimal native Zenodo REST API v1 client for `lotus archive --publish`.
//!
//! Replaces `zenodo-rs` (which pulled `chrono` and a second rustls crypto backend)
//! with the small subset of Zenodo's deposition API the archive subcommand uses:
//! creating or versioning a draft, updating metadata, replacing files in the
//! draft bucket, and publishing.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, bail};
use reqwest::Method;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

const PRODUCTION_API_URL: &str = "https://zenodo.org/api";
const SANDBOX_API_URL: &str = "https://sandbox.zenodo.org/api";
const USER_AGENT: &str = concat!("lotus-cli/", env!("CARGO_PKG_VERSION"));
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(500);
const DEFAULT_MAX_POLLS: usize = 24;
const MAX_TRANSIENT_ATTEMPTS: usize = 3;

/// Numeric Zenodo deposition or published record identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DepositionId(pub u64);

impl fmt::Display for DepositionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Validated file queued for upload to a Zenodo draft bucket.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadFile {
    /// Target filename inside the Zenodo bucket.
    pub filename: String,
    /// Local path to read bytes from.
    pub path: PathBuf,
}

/// Validates and deduplicates upload `(filename, path)` entries before transfer.
///
/// # Errors
/// Returns an error if a filename is empty, contains path separators or
/// dot-segments, or repeats an earlier filename.
pub fn prepare_uploads<'a>(
    entries: impl IntoIterator<Item = (String, &'a Path)>,
) -> anyhow::Result<Vec<UploadFile>> {
    let mut seen = BTreeSet::new();
    let mut files = Vec::new();
    for (filename, path) in entries {
        if filename.is_empty()
            || filename == "."
            || filename == ".."
            || filename.contains('/')
            || filename.contains('\\')
        {
            bail!("invalid upload filename: {filename:?}");
        }
        if !seen.insert(filename.clone()) {
            bail!("duplicate upload filename: {filename}");
        }
        files.push(UploadFile {
            filename,
            path: path.to_path_buf(),
        });
    }
    Ok(files)
}

/// Creator metadata entry sent to Zenodo.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Creator {
    /// Creator display name (`"Family, Given"` or organization name).
    pub name: String,
}

/// Related identifier metadata entry sent to Zenodo.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RelatedIdentifier {
    /// Identifier value (such as a pinned source-code commit URL).
    pub identifier: String,
    /// Relation to the deposit (e.g. `"isDerivedFrom"`).
    pub relation: &'static str,
    /// Identifier scheme (e.g. `"url"`).
    pub scheme: &'static str,
    /// Related resource type (e.g. `"software"`).
    pub resource_type: &'static str,
}

/// Target community identifier sent to Zenodo.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Community {
    /// Community slug on Zenodo.
    pub identifier: &'static str,
}

/// Zenodo deposition metadata payload (`PUT /api/deposit/depositions/{id}`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DepositMetadata {
    /// Record title.
    pub title: String,
    /// Upload type vocabulary string (e.g. `"dataset"`).
    pub upload_type: &'static str,
    /// Publication calendar date (`YYYY-MM-DD`).
    pub publication_date: String,
    /// HTML description body serialized under Zenodo's `description` key.
    #[serde(rename = "description")]
    pub description_html: String,
    /// Ordered list of creators.
    pub creators: Vec<Creator>,
    /// Access right vocabulary string (e.g. `"open"`).
    pub access_right: &'static str,
    /// Open-access license identifier (e.g. `"cc-by-4.0"`).
    pub license: &'static str,
    /// Subject keywords.
    pub keywords: Vec<String>,
    /// Linked identifiers.
    pub related_identifiers: Vec<RelatedIdentifier>,
    /// Target Zenodo communities.
    pub communities: Vec<Community>,
}

/// Published record summary returned after publishing a deposition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublishedRecord {
    /// Published Zenodo record identifier.
    pub record_id: DepositionId,
}

#[derive(Debug, Deserialize)]
struct Deposition {
    id: u64,
    #[serde(default)]
    submitted: bool,
    #[serde(default)]
    record_id: Option<u64>,
    #[serde(default)]
    files: Vec<DepositionFile>,
    #[serde(default)]
    links: DepositionLinks,
}

#[derive(Debug, Deserialize)]
struct DepositionFile {
    id: String,
}

#[derive(Debug, Default, Deserialize)]
struct DepositionLinks {
    #[serde(rename = "self", default)]
    self_: Option<String>,
    #[serde(default)]
    bucket: Option<String>,
    #[serde(default)]
    latest_draft: Option<String>,
    #[serde(default)]
    latest: Option<String>,
}

/// Authenticated HTTP client for Zenodo's deposition REST API.
pub struct ZenodoClient {
    http: reqwest::Client,
    base_url: String,
    token: String,
    poll_interval: Duration,
    max_polls: usize,
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

    fn from_env_with(
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
    pub(crate) const fn with_fast_timings(mut self) -> Self {
        self.poll_interval = Duration::from_millis(1);
        self.max_polls = 10;
        self.retry_delay = Duration::from_millis(1);
        self
    }

    /// Creates a new deposition (or versions `root_id`), updates metadata,
    /// replaces draft files with `uploads`, and publishes the record.
    ///
    /// # Errors
    /// Returns an error if any step in the Zenodo deposition lifecycle fails.
    pub async fn publish_dataset(
        &self,
        root_id: Option<DepositionId>,
        metadata: &DepositMetadata,
        uploads: &[UploadFile],
    ) -> anyhow::Result<PublishedRecord> {
        let draft = match root_id {
            Some(id) => self
                .ensure_editable_draft(id)
                .await
                .context("ensure editable draft")?,
            None => self
                .create_deposition()
                .await
                .context("create Zenodo deposition")?,
        };
        let draft_id = DepositionId(draft.id);
        let updated = self
            .update_metadata(draft_id, metadata)
            .await
            .context("update Zenodo metadata")?;
        let refreshed = if updated.links.bucket.is_some() {
            updated
        } else {
            self.get_deposition(draft_id)
                .await
                .context("refresh Zenodo draft")?
        };
        let bucket = refreshed
            .links
            .bucket
            .as_deref()
            .context("Zenodo draft is missing links.bucket")?;

        for existing in &refreshed.files {
            self.delete_file(draft_id, &existing.id)
                .await
                .with_context(|| format!("delete existing draft file {}", existing.id))?;
        }

        for file in uploads {
            self.upload_file(bucket, file)
                .await
                .with_context(|| format!("upload {}", file.filename))?;
        }

        let published = self
            .publish_and_wait(draft_id)
            .await
            .context("publish Zenodo deposition")?;
        let record_id = DepositionId(published.record_id.unwrap_or(published.id));
        Ok(PublishedRecord { record_id })
    }

    async fn create_deposition(&self) -> anyhow::Result<Deposition> {
        let url = format!("{}/deposit/depositions", self.base_url);
        let response = self
            .request(Method::POST, &url)
            .json(&serde_json::json!({}))
            .send()
            .await
            .context("POST deposit/depositions")?;
        parse_json_response(response, "create deposition").await
    }

    async fn get_deposition(&self, id: DepositionId) -> anyhow::Result<Deposition> {
        let url = format!("{}/deposit/depositions/{id}", self.base_url);
        self.get_deposition_by_url(&url).await
    }

    async fn get_deposition_by_url(&self, url: &str) -> anyhow::Result<Deposition> {
        self.send_with_retries(
            || self.request(Method::GET, url),
            "GET deposition",
            parse_json_response::<Deposition>,
        )
        .await
    }

    async fn ensure_editable_draft(&self, root_id: DepositionId) -> anyhow::Result<Deposition> {
        let deposition = self.get_deposition(root_id).await?;
        if !deposition.submitted {
            return Ok(deposition);
        }

        let latest = self.resolve_latest_published(deposition).await?;
        let new_ver_url = format!(
            "{}/deposit/depositions/{}/actions/newversion",
            self.base_url, latest.id
        );
        let response = self
            .request(Method::POST, &new_ver_url)
            .send()
            .await
            .context("POST actions/newversion")?;
        let versioned =
            parse_action_deposition(response, || self.get_deposition(DepositionId(latest.id)))
                .await?;
        let draft_url = versioned
            .links
            .latest_draft
            .context("Zenodo newversion response is missing links.latest_draft")?;

        for _ in 0..self.max_polls {
            if let Ok(draft) = self.get_deposition_by_url(&draft_url).await
                && !draft.submitted
                && draft.links.bucket.is_some()
            {
                return Ok(draft);
            }
            tokio::time::sleep(self.poll_interval).await;
        }
        bail!("timed out waiting for Zenodo editable draft at {draft_url}");
    }

    async fn resolve_latest_published(&self, deposition: Deposition) -> anyhow::Result<Deposition> {
        if let Some(latest_url) = deposition.links.latest.as_deref()
            && deposition.links.self_.as_deref() != Some(latest_url)
        {
            if latest_url.contains("/deposit/depositions/") {
                return self.get_deposition_by_url(latest_url).await;
            }
            if let Some(record_id) = parse_record_id_from_url(latest_url) {
                return self.get_deposition(DepositionId(record_id)).await;
            }
        }
        Ok(deposition)
    }

    async fn update_metadata(
        &self,
        id: DepositionId,
        metadata: &DepositMetadata,
    ) -> anyhow::Result<Deposition> {
        #[derive(Serialize)]
        struct Payload<'a> {
            metadata: &'a DepositMetadata,
        }
        let url = format!("{}/deposit/depositions/{id}", self.base_url);
        let response = self
            .request(Method::PUT, &url)
            .json(&Payload { metadata })
            .send()
            .await
            .context("PUT deposition metadata")?;
        parse_json_response(response, "update metadata").await
    }

    async fn delete_file(&self, id: DepositionId, file_id: &str) -> anyhow::Result<()> {
        let url = format!(
            "{}/deposit/depositions/{id}/files/{}",
            self.base_url,
            lotus_model::percent_encode(file_id)
        );
        let response = self
            .request(Method::DELETE, &url)
            .send()
            .await
            .context("DELETE draft file")?;
        ensure_success(response, "delete draft file").await?;
        Ok(())
    }

    async fn upload_file(&self, bucket_url: &str, file: &UploadFile) -> anyhow::Result<()> {
        let bytes = std::fs::read(&file.path)
            .with_context(|| format!("read upload file {}", file.path.display()))?;
        let len = bytes.len();
        let upload_url = format!(
            "{}/{}",
            bucket_url.trim_end_matches('/'),
            lotus_model::percent_encode(&file.filename)
        );
        self.send_with_retries(
            || {
                self.request(Method::PUT, &upload_url)
                    .header(CONTENT_TYPE, "application/octet-stream")
                    .header(CONTENT_LENGTH, len)
                    .body(bytes.clone())
            },
            "PUT bucket object",
            |resp, op| async move {
                ensure_success(resp, op).await?;
                Ok(())
            },
        )
        .await
    }

    async fn publish_and_wait(&self, id: DepositionId) -> anyhow::Result<Deposition> {
        let url = format!("{}/deposit/depositions/{id}/actions/publish", self.base_url);
        let response = self
            .request(Method::POST, &url)
            .send()
            .await
            .context("POST actions/publish")?;
        let initial = parse_action_deposition(response, || self.get_deposition(id)).await?;
        if initial.submitted {
            return Ok(initial);
        }
        for _ in 0..self.max_polls {
            tokio::time::sleep(self.poll_interval).await;
            let current = self.get_deposition(id).await?;
            if current.submitted {
                return Ok(current);
            }
        }
        bail!("timed out waiting for Zenodo deposition {id} to finish publishing");
    }

    fn request(&self, method: Method, url: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(ACCEPT, "application/json")
    }

    async fn send_with_retries<T, F, Fut>(
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

fn read_non_empty_env(
    name: &str,
    get_env: &mut impl FnMut(&str) -> Result<String, std::env::VarError>,
) -> anyhow::Result<Option<String>> {
    match get_env(name) {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        Ok(_) | Err(std::env::VarError::NotPresent) => Ok(None),
        Err(err) => bail!("cannot read {name}: {err}"),
    }
}

fn parse_record_id_from_url(url: &str) -> Option<u64> {
    let (_, tail) = url.rsplit_once("/records/")?;
    let digits = tail.split(['/', '?', '#']).next()?;
    digits.parse::<u64>().ok()
}

async fn ensure_success(
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

async fn parse_json_response<T: serde::de::DeserializeOwned>(
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

async fn parse_action_deposition<F, Fut>(
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

#[cfg(test)]
#[path = "zenodo/tests.rs"]
mod tests;
