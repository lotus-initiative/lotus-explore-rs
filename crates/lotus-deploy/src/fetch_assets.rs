// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Fetch the external frontend assets used by the web client.
//!
//! Downloads the configured RDKit.js and Scholia Citation.js bundles into
//! `public/assets/vendor`, then downloads and slims the standalone Ketcher
//! editor bundle. Run from the app crate directory so the relative output paths
//! land in the app's `public` tree.
//!
//! # Environment
//!
//! * `CURATION_ASSET_DIR` — curation asset output directory (default
//!   `public/assets/vendor`).
//! * `CURATION_ASSET_STATE` — cache-state path (default
//!   `target/lotus-assets-state`).
//! * `KETCHER_VERSION` — release tag or `latest` (default `latest`).
//! * `KETCHER_DIR` — output directory (default `public/assets/ketcher`).
//! * `KETCHER_URL` — fully override the release URL.
//! * `RDKIT_VERSION` — npm version or `latest` (default `latest`).
//! * `CITATION_JS_REF` — Scholia branch, tag, or commit (default `main`).

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::{self, BufWriter, Cursor, Write};
use std::path::{Path, PathBuf};

use reqwest::blocking::Client;
use serde_json::Value;
use zip::ZipArchive;

const DEFAULT_KETCHER_VERSION: &str = "latest";
const DEFAULT_RDKIT_VERSION: &str = "latest";
const DEFAULT_CITATION_JS_REF: &str = "main";
const KETCHER_OWNER_REPO: &str = "epam/ketcher";
const KETCHER_LATEST_URL: &str = "https://github.com/epam/ketcher/releases/latest";
const RDKIT_LATEST_METADATA: &str = "https://unpkg.com/@rdkit/rdkit@latest/?meta";
const RDKIT_LICENSE_URL: &str = "https://raw.githubusercontent.com/rdkit/rdkit/master/license.txt";
const SCHOLIA_REPO: &str = "WDscholia/scholia";
const SCHOLIA_RAW_URL: &str = "https://raw.githubusercontent.com/WDscholia/scholia";
const CITATION_LICENSE_URL: &str =
    "https://raw.githubusercontent.com/citation-js/citation-js/main/LICENSE.md";
const DEFAULT_DIR: &str = "public/assets/ketcher";
const DEFAULT_CURATION_DIR: &str = "public/assets/vendor";
const DEFAULT_CURATION_STATE: &str = "target/lotus-assets-state";

fn setting(name: &str, default: &str) -> String {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_owned())
}

/// Unused standalone "entry" bundles (and their license files) that ketcher's
/// `index.html` never references — only `main.<hash>.js` is loaded by the
/// editor iframe. Matches the original shell helper's `rm` globs: only the
/// `closable`/`duo`/`popup` JavaScript bundles and their `.LICENSE.txt` are
/// dropped; the small mode-specific `*.html`/`*.css` entry points are kept.
#[must_use]
fn is_unused_entry(name: &str) -> bool {
    let Some(file_name) = name.rsplit('/').next() else {
        return false;
    };
    let is_entry_bundle = file_name.starts_with("closable.")
        || file_name.starts_with("duo.")
        || file_name.starts_with("popup.");
    is_entry_bundle
        && (Path::new(file_name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("js"))
            || file_name.ends_with(".js.LICENSE.txt"))
}

/// macOS zip metadata that must never be extracted: the `__MACOSX/` tree and
/// `._`-prefixed resource forks. The ketcher release zip ships these
/// (it was archived on macOS), and dioxus-cli's asset copier aborts on them
/// with "stream did not contain valid UTF-8" / esbuild `Unexpected "\x00"`.
/// The original shell helper avoided this implicitly via `cp -r standalone/*`;
/// this makes it explicit and keeps the resource forks out of `public/assets`.
#[must_use]
fn is_macos_junk(name: &str) -> bool {
    if name == "__MACOSX" || name.starts_with("__MACOSX/") {
        return true;
    }
    let Some(file_name) = name.rsplit('/').next() else {
        return false;
    };
    file_name.starts_with("._")
}

#[must_use]
fn release_url(version: &str) -> String {
    format!(
        "https://github.com/{KETCHER_OWNER_REPO}/releases/download/v{version}/ketcher-standalone-{version}.zip"
    )
}

fn normalize_version(version: &str) -> String {
    version.strip_prefix('v').unwrap_or(version).to_owned()
}

fn read_json(client: &Client, url: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let response = client.get(url).send()?;
    if !response.status().is_success() {
        return Err(format!("HTTP {} fetching {url}", response.status()).into());
    }
    Ok(serde_json::from_slice(&response.bytes()?)?)
}

fn fetch_file(
    client: &Client,
    url: &str,
    destination: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Downloading {url} ...");
    let response = client.get(url).send()?;
    if !response.status().is_success() {
        return Err(format!("HTTP {} fetching {url}", response.status()).into());
    }
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

fn resolve_ketcher_version(client: &Client) -> Result<String, Box<dyn std::error::Error>> {
    let requested = setting("KETCHER_VERSION", DEFAULT_KETCHER_VERSION);
    if requested != "latest" {
        return Ok(normalize_version(&requested));
    }
    let response = client.get(KETCHER_LATEST_URL).send()?;
    if !response.status().is_success() {
        return Err(format!("HTTP {} fetching {KETCHER_LATEST_URL}", response.status()).into());
    }
    let Some(tag) = response
        .url()
        .path()
        .split_once("/tag/")
        .map(|(_, tag)| tag)
    else {
        return Err("Ketcher latest URL did not resolve to a release tag".into());
    };
    Ok(normalize_version(tag))
}

fn resolve_rdkit_version(client: &Client) -> Result<String, Box<dyn std::error::Error>> {
    let requested = setting("RDKIT_VERSION", DEFAULT_RDKIT_VERSION);
    if requested != "latest" {
        return Ok(requested);
    }
    let metadata = read_json(client, RDKIT_LATEST_METADATA)?;
    let Some(version) = metadata.get("version").and_then(Value::as_str) else {
        return Err("RDKit package metadata has no version".into());
    };
    Ok(version.to_owned())
}

fn github_commit(body: &str) -> Option<String> {
    let marker = "Grit::Commit/";
    let start = body.find(marker)? + marker.len();
    let commit = body[start..].chars().take(40).collect::<String>();
    (commit.len() == 40
        && commit
            .chars()
            .all(|character| character.is_ascii_hexdigit()))
    .then_some(commit)
}

fn resolve_github_ref(
    client: &Client,
    repository: &str,
    requested: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    if requested.len() == 40
        && requested
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Ok(requested.to_owned());
    }

    let url = format!("https://github.com/{repository}/commits/{requested}.atom");
    let response = client.get(&url).send()?;
    if !response.status().is_success() {
        return Err(format!("HTTP {} fetching {url}", response.status()).into());
    }
    let bytes = response.bytes()?;
    let body = String::from_utf8_lossy(&bytes);
    github_commit(&body)
        .ok_or_else(|| format!("GitHub ref {repository}/{requested} has no commit id").into())
}

fn fetch_curation_assets(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(setting("CURATION_ASSET_DIR", DEFAULT_CURATION_DIR));
    let rdkit_version = resolve_rdkit_version(client)?;
    let citation_commit = resolve_github_ref(
        client,
        SCHOLIA_REPO,
        &setting("CITATION_JS_REF", DEFAULT_CITATION_JS_REF),
    )?;
    let state = format!("rdkit={rdkit_version}\nscholia={citation_commit}\n");
    let state_path = PathBuf::from(setting("CURATION_ASSET_STATE", DEFAULT_CURATION_STATE));
    let required_paths = [
        "rdkit/RDKit_minimal.js",
        "rdkit/RDKit_minimal.wasm",
        "rdkit/LICENSE.txt",
        "citation-js/citation.js",
        "citation-js/LICENSE.scholia.txt",
        "citation-js/LICENSE.citation-js.txt",
    ];
    let cached = fs::read_to_string(&state_path).is_ok_and(|current| {
        current == state && required_paths.iter().all(|path| root.join(path).is_file())
    });
    if cached {
        println!("✓ RDKit {rdkit_version} and Scholia {citation_commit} already present");
        return Ok(());
    }

    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;

    let rdkit_dist = format!("https://unpkg.com/@rdkit/rdkit@{rdkit_version}/dist");
    let scholia = format!("{SCHOLIA_RAW_URL}/{citation_commit}");
    let assets = [
        (
            format!("{rdkit_dist}/RDKit_minimal.js"),
            "rdkit/RDKit_minimal.js",
        ),
        (
            format!("{rdkit_dist}/RDKit_minimal.wasm"),
            "rdkit/RDKit_minimal.wasm",
        ),
        (RDKIT_LICENSE_URL.to_owned(), "rdkit/LICENSE.txt"),
        (
            format!("{scholia}/scholia/app/static/js/citation.js"),
            "citation-js/citation.js",
        ),
        (
            format!("{scholia}/LICENSE"),
            "citation-js/LICENSE.scholia.txt",
        ),
        (
            CITATION_LICENSE_URL.to_owned(),
            "citation-js/LICENSE.citation-js.txt",
        ),
    ];

    println!("Using RDKit {rdkit_version} and Scholia {citation_commit}");
    for (url, relative_path) in assets {
        fetch_file(client, &url, &root.join(relative_path))?;
    }
    if let Some(parent) = state_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(state_path, state)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder().build()?;
    fetch_curation_assets(&client)?;

    let version = resolve_ketcher_version(&client)?;
    let ketcher_dir = PathBuf::from(setting("KETCHER_DIR", DEFAULT_DIR));
    let index_html = ketcher_dir.join("index.html");

    if index_html.is_file() {
        let index = fs::read_to_string(&index_html)?;
        if index.contains(&format!("Ketcher v{version}")) {
            println!(
                "✓ Ketcher v{version} already present in {}",
                ketcher_dir.display()
            );
            return Ok(());
        }
        fs::remove_dir_all(&ketcher_dir)?;
        println!("Updating Ketcher to v{version}");
    }

    let url = env::var("KETCHER_URL").unwrap_or_else(|_| release_url(&version));
    println!("Downloading Ketcher v{version} from {url} ...");

    let response = client.get(&url).send()?;
    if !response.status().is_success() {
        return Err(format!("HTTP {} fetching {url}", response.status()).into());
    }
    let bytes = response.bytes()?;
    let total = bytes.len();
    println!("  downloaded {total} bytes");

    fs::create_dir_all(&ketcher_dir)?;
    let mut archive = ZipArchive::new(Cursor::new(bytes.to_vec()))?;

    let mut top_levels: BTreeSet<String> = BTreeSet::new();
    for i in 0..archive.len() {
        let name = archive.by_index(i)?.name().to_string();
        if is_macos_junk(&name) {
            continue;
        }
        let Some(first) = name.split('/').next() else {
            continue;
        };
        if !first.is_empty() {
            top_levels.insert(first.to_string());
        }
    }
    let strip_prefix = if top_levels.len() == 1 {
        top_levels.into_iter().next()
    } else {
        None
    };

    let mut entries = 0u64;
    let mut skipped_bytes = 0u64;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;

        if is_macos_junk(file.name()) {
            continue;
        }
        if is_unused_entry(file.name()) {
            skipped_bytes += file.size();
            continue;
        }

        let rel = strip_prefix
            .as_deref()
            .and_then(|prefix| file.name().strip_prefix(prefix).map(str::to_string))
            .unwrap_or_else(|| file.name().to_string());
        let rel = rel.trim_start_matches('/');
        if rel.is_empty() || rel.contains("..") || rel.starts_with('/') {
            continue;
        }

        let out_path = ketcher_dir.join(rel);
        if file.is_dir() {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        let Some(parent) = out_path.parent() else {
            continue;
        };
        fs::create_dir_all(parent)?;
        let mut out = BufWriter::new(fs::File::create(&out_path)?);
        io::copy(&mut file, &mut out)?;
        out.flush()?;
        entries += 1;
    }

    println!("  extracted {entries} file(s) to {}", ketcher_dir.display());
    if skipped_bytes > 0 {
        println!("  skipped {skipped_bytes} bytes of unused entry bundles (closable/duo/popup)");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_unused_entries() {
        assert!(is_unused_entry("standalone/static/js/closable.5cead650.js"));
        assert!(is_unused_entry("standalone/static/js/duo.546fbaab.js"));
        assert!(is_unused_entry("standalone/static/js/popup.ec23766a.js"));
        assert!(is_unused_entry(
            "standalone/static/js/closable.5cead650.js.LICENSE.txt"
        ));
        assert!(is_unused_entry(
            "standalone/static/js/duo.546fbaab.js.LICENSE.txt"
        ));
        assert!(!is_unused_entry("standalone/static/js/main.cb80d824.js"));
        assert!(!is_unused_entry(
            "standalone/static/js/157.7de4e426.chunk.js"
        ));
        assert!(!is_unused_entry(
            "standalone/static/js/622.ed91ac0.chunk.js.LICENSE.txt"
        ));
        assert!(!is_unused_entry("standalone/index.html"));
        assert!(!is_unused_entry("standalone/static/css/main.9cca8bc6.css"));
        assert!(!is_unused_entry("standalone/duo.html"));
        assert!(!is_unused_entry(
            "standalone/static/css/closable.9cca8bc6.css"
        ));
        assert!(!is_unused_entry("standalone/._duo.546fbaab.js"));
    }

    #[test]
    fn classifies_macos_junk() {
        assert!(is_macos_junk("__MACOSX"));
        assert!(is_macos_junk("__MACOSX/standalone/._index.html"));
        assert!(is_macos_junk(
            "__MACOSX/standalone/static/js/._duo.546fbaab.js"
        ));
        assert!(is_macos_junk(
            "__MACOSX/standalone/static/js/._asset-manifest.json"
        ));
        assert!(!is_macos_junk("standalone/index.html"));
        assert!(!is_macos_junk("standalone/static/js/main.cb80d824.js"));
        assert!(!is_macos_junk("standalone/static/js/duo.546fbaab.js"));
    }

    #[test]
    fn release_url_points_at_github_releases() {
        assert_eq!(
            release_url("3.18.0"),
            "https://github.com/epam/ketcher/releases/download/v3.18.0/ketcher-standalone-3.18.0.zip"
        );
        assert_eq!(
            release_url("3.10.0"),
            "https://github.com/epam/ketcher/releases/download/v3.10.0/ketcher-standalone-3.10.0.zip"
        );
    }

    #[test]
    fn normalizes_version_prefixes() {
        assert_eq!(normalize_version("v3.18.0"), "3.18.0");
        assert_eq!(normalize_version("3.18.0"), "3.18.0");
    }

    #[test]
    fn extracts_github_commit_from_atom_feed() {
        let body =
            "<id>tag:github.com,2008:Grit::Commit/0123456789abcdef0123456789abcdef01234567</id>";
        assert_eq!(
            github_commit(body).as_deref(),
            Some("0123456789abcdef0123456789abcdef01234567")
        );
        assert!(github_commit("<id>not-a-commit</id>").is_none());
    }
}
