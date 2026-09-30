// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Environment settings and the HTTP calls every asset fetch shares.
//!
//! The rules here are the ones that decide what ends up in the repository, so
//! they are stated once rather than at each fetch site: a non-2xx is an error
//! even when it carries a plausible body, and an empty body is never an asset.

use std::env;
use std::fs;
use std::path::Path;

use reqwest::blocking::Client;
use serde_json::Value;

#[cfg(test)]
mod tests;

/// A blank value counts as unset.
///
/// Split from [`setting`] so the distinction is reachable without mutating the
/// process environment, which is global: an exported `KETCHER_VERSION=""` in CI
/// would otherwise be taken as a path of zero length.
fn setting_or(value: Option<String>, default: &str) -> String {
    value
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_owned())
}

/// Read an environment setting, falling back to `default`.
#[must_use]
pub fn setting(name: &str, default: &str) -> String {
    setting_or(env::var(name).ok(), default)
}

/// Reject a response that is not 2xx, naming the URL.
///
/// Shared by every fetch here because a 404 from a CDN is a perfectly good body
/// to write to disk, and the asset then looks present until the deployed site
/// 404s on it.
fn check_status(status: reqwest::StatusCode, url: &str) -> Result<(), Box<dyn std::error::Error>> {
    if status.is_success() {
        Ok(())
    } else {
        Err(format!("HTTP {status} fetching {url}").into())
    }
}

/// GET a URL and parse the body as JSON.
///
/// # Errors
/// Returns the transport failure, a non-2xx status, or a body that is not JSON.
pub fn read_json(client: &Client, url: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let response = client.get(url).send()?;
    check_status(response.status(), url)?;
    Ok(serde_json::from_slice(&response.bytes()?)?)
}

/// GET a URL and write the body to `destination`, creating parent directories.
///
/// An empty body is an error: it is how a captive portal or a truncated CDN
/// response presents itself, and writing it produces an asset that is present,
/// the right size, and wrong.
///
/// # Errors
/// Returns the transport failure, a non-2xx status, an empty body, or the
/// filesystem failure from creating or writing the destination.
pub fn fetch_file(
    client: &Client,
    url: &str,
    destination: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Downloading {url} ...");
    let response = client.get(url).send()?;
    check_status(response.status(), url)?;
    let bytes = response.bytes()?;
    if bytes.is_empty() {
        return Err(format!("empty response fetching {url}").into());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(destination, bytes)?;
    Ok(())
}

/// A full 40-character git object id.
///
/// Shared because it answers two different questions -- "did the feed give me a
/// commit id" and "was the caller pinning one" -- and the two had drifted into
/// separate copies of the same check.
fn is_commit_id(candidate: &str) -> bool {
    candidate.len() == 40 && candidate.chars().all(|c| c.is_ascii_hexdigit())
}

/// Read the commit id out of a GitHub atom feed for a branch or tag.
fn github_commit(body: &str) -> Option<String> {
    let marker = "Grit::Commit/";
    let start = body.find(marker)? + marker.len();
    let commit = body[start..].chars().take(40).collect::<String>();
    is_commit_id(&commit).then_some(commit)
}

/// Resolve `requested` to a commit id, following the GitHub feed when it is a
/// branch or tag rather than a full id.
///
/// # Errors
/// Errors when the feed 404s or carries no commit id. Vendoring an arbitrary
/// snapshot instead would put unreviewed bytes in the repository.
pub fn resolve_github_ref(
    client: &Client,
    base_url: &str,
    repository: &str,
    requested: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    if is_commit_id(requested) {
        return Ok(requested.to_owned());
    }

    let url = format!("{base_url}/{repository}/commits/{requested}.atom");
    let response = client.get(&url).send()?;
    check_status(response.status(), &url)?;
    let bytes = response.bytes()?;
    let body = String::from_utf8_lossy(&bytes);
    github_commit(&body)
        .ok_or_else(|| format!("GitHub ref {repository}/{requested} has no commit id").into())
}

/// Resolve a version that may be `latest`, `v1.2.3` or a bare `1.2.3`.
///
/// # Errors
/// Errors when the latest-release lookup 404s or resolves to a URL that is not
/// a release. Guessing a version would vendor the wrong bundle.
pub fn resolve_tagged_version(
    client: &Client,
    latest_url: &str,
    requested: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    if requested != "latest" {
        return Ok(normalize_version(requested));
    }
    let response = client.get(latest_url).send()?;
    check_status(response.status(), latest_url)?;
    let Some(tag) = response
        .url()
        .path()
        .split_once("/tag/")
        // GitHub redirects to `/releases/tag/v2.3.4`, but a trailing slash
        // survives in the path and would be pasted into the download URL.
        .map(|(_, tag)| tag.trim_end_matches('/'))
    else {
        return Err(
            format!("Ketcher latest URL {latest_url} did not resolve to a release tag").into(),
        );
    };
    Ok(normalize_version(tag))
}

/// Strip the leading `v` GitHub puts on release tags, so a pinned and a resolved
/// version compare equal.
fn normalize_version(version: &str) -> String {
    version.strip_prefix('v').unwrap_or(version).to_owned()
}

/// Resolve a version that may be `latest`, reading the current one out of a JSON
/// package-metadata document.
///
/// # Errors
/// Errors when the metadata request fails or the document has no `version`.
pub fn resolve_metadata_version(
    client: &Client,
    metadata_url: &str,
    requested: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    if requested != "latest" {
        return Ok(requested.to_owned());
    }
    let metadata = read_json(client, metadata_url)?;
    metadata
        .get("version")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("package metadata at {metadata_url} has no version").into())
}
