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

/// How many times a request is attempted before it is called failed.
///
/// Three, with the wait doubling between them. This binary pulls ~115 MB from
/// four hosts -- unpkg, GitHub releases, raw.githubusercontent.com, and a
/// licence file on each -- so a run makes a dozen requests and every one of them
/// is an opportunity for a connection reset, a CDN 5xx, or a rate limit. One
/// attempt per request made the whole build fail intermittently on a transient
/// blip, and a build that fails at random is a build people stop reading.
const ATTEMPTS: u32 = 3;

/// Base wait before the second attempt. Doubled each time, so the three waits are
/// 1 s, 2 s, 4 s -- long enough for a rate limit to clear, short enough that a
/// genuinely broken URL fails in under a minute.
///
/// A test that makes a request fail needs to sit through that, so under `cfg(test)`
/// it is short enough to run. The retry *count* is unchanged, so what the tests
/// exercise is the same number of attempts.
#[cfg(not(test))]
const BACKOFF: std::time::Duration = std::time::Duration::from_secs(1);
#[cfg(test)]
const BACKOFF: std::time::Duration = std::time::Duration::from_millis(1);

/// Whether a failure is worth trying again.
///
/// A transport error or a 5xx is the server or the network, not the request: the
/// same URL usually works a moment later. A 4xx is the URL, and repeating it
/// three times only triples the wait before the same answer. `429` is the one
/// 4xx that clears on its own.
#[must_use]
fn is_retryable(status: Option<reqwest::StatusCode>) -> bool {
    // `None` means the request never completed.
    status.is_none_or(|status| {
        status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS
    })
}

/// GET a URL, retrying a transient failure, and hand back the response.
///
/// Every request in this binary goes through here, so the retry policy is stated
/// once rather than at each call site.
///
/// # Errors
/// Returns the last failure once the attempts are spent: the transport error, or
/// the status the endpoint kept answering with.
pub fn get(
    client: &Client,
    url: &str,
) -> Result<reqwest::blocking::Response, Box<dyn std::error::Error>> {
    let mut wait = BACKOFF;
    for attempt in 1..=ATTEMPTS {
        match client.get(url).send() {
            Ok(response) if response.status().is_success() => return Ok(response),
            Ok(response) => {
                let status = response.status();
                if !is_retryable(Some(status)) {
                    return Err(format!("HTTP {status} fetching {url}").into());
                }
                if attempt == ATTEMPTS {
                    return Err(format!("HTTP {status} fetching {url}").into());
                }
                eprintln!("  {status} from {url}, retrying in {wait:?}");
            }
            Err(error) => {
                if attempt == ATTEMPTS {
                    return Err(format!("could not reach {url}: {error}").into());
                }
                eprintln!("  {url}: {error}, retrying in {wait:?}");
            }
        }
        std::thread::sleep(wait);
        wait *= 2;
    }
    unreachable!("the loop returns on the last attempt")
}

/// GET a URL and parse the body as JSON.
///
/// # Errors
/// Returns the transport failure, a non-2xx status, or a body that is not JSON.
pub fn read_json(client: &Client, url: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let response = get(client, url)?;
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
    let response = get(client, url)?;
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
    let response = get(client, &url)?;
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
    let response = get(client, latest_url)?;
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
