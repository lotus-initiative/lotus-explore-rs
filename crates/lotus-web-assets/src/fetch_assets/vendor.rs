// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The vendored curation assets: `RDKit` and the Citation.js build Scholia uses.
//!
//! Each family is cached against a version recorded in a small `key=value` state
//! file, so a run that changes nothing downloads nothing. The two are invalidated
//! independently: Citation.js moving to a new commit must not cost a re-download
//! of the 7 MB `RDKit` wasm module, and vice versa.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use reqwest::blocking::Client;

use super::http::{fetch_file, resolve_github_ref, resolve_metadata_version, setting};

const DEFAULT_CURATION_DIR: &str = "public/assets/vendor";
const DEFAULT_CURATION_STATE: &str = "target/lotus-assets-state";
const RDKIT_LATEST_METADATA: &str = "https://unpkg.com/@rdkit/rdkit@latest/?meta";
const RDKIT_LICENSE_URL: &str = "https://raw.githubusercontent.com/rdkit/rdkit/master/license.txt";
const SCHOLIA_REPO: &str = "WDscholia/scholia";
const SCHOLIA_RAW_URL: &str = "https://raw.githubusercontent.com/WDscholia/scholia";
const GITHUB_BASE_URL: &str = "https://github.com";
const CITATION_LICENSE_URL: &str =
    "https://raw.githubusercontent.com/citation-js/citation-js/main/LICENSE.md";

#[cfg(test)]
mod tests;

/// One vendored family of third-party assets, checked, invalidated and logged on
/// its own.
#[derive(Debug)]
struct VendoredAsset {
    /// Human-readable name, for the log.
    name: &'static str,
    /// Key this asset owns in the state file.
    state_key: &'static str,
    /// Directory under the curation root that this asset owns, and the only one
    /// removed when this asset is stale.
    pub dir: &'static str,
    /// The resolved version or commit.
    version: String,
    /// `(url, path within `dir`)`. Licences are listed deliberately, so a missing
    /// one invalidates the asset rather than shipping unlicensed bytes.
    files: Vec<(String, String)>,
}

impl VendoredAsset {
    /// Every file this asset must have on disk to count as cached.
    fn paths(&self, root: &Path) -> Vec<PathBuf> {
        self.files
            .iter()
            .map(|(_, relative)| root.join(self.dir).join(relative))
            .collect()
    }

    /// Whether this asset can be reused as-is.
    ///
    /// Both halves matter: a partial download leaves the recorded version in
    /// place with files missing, so a version match on its own would trust a
    /// directory that is not there.
    fn is_current(&self, recorded: Option<&str>, all_files_present: bool) -> bool {
        recorded == Some(self.version.as_str()) && all_files_present
    }

    /// Reuse the copy on disk, or replace it and record the new version.
    ///
    /// The version is recorded only after every file has landed, so an interrupted
    /// fetch is retried next time instead of being trusted.
    fn refresh(
        &self,
        client: &Client,
        root: &Path,
        state: &mut BTreeMap<String, String>,
        state_path: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.is_current(
            state.get(self.state_key).map(String::as_str),
            self.paths(root).iter().all(|path| path.is_file()),
        ) {
            println!(
                "✓ {} {} already present ({} files in {}/{})",
                self.name,
                self.version,
                self.files.len(),
                root.display(),
                self.dir
            );
            return Ok(());
        }

        // Only this asset's own directory goes, so a stale Citation.js never
        // costs a re-download of RDKit and vice versa.
        let dir = root.join(self.dir);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        fs::create_dir_all(&dir)?;
        println!(
            "↓ {} {} is not cached — fetching {} files into {}/{}",
            self.name,
            self.version,
            self.files.len(),
            root.display(),
            self.dir
        );
        for (url, relative_path) in &self.files {
            fetch_file(client, url, &dir.join(relative_path))?;
        }

        state.insert(self.state_key.to_owned(), self.version.clone());
        write_state(state_path, state)?;
        Ok(())
    }
}

/// The two families, with the versions just resolved for them.
fn assets(rdkit_version: String, citation_commit: String) -> [VendoredAsset; 2] {
    let rdkit_dist = format!("https://unpkg.com/@rdkit/rdkit@{rdkit_version}/dist");
    let scholia = format!("{SCHOLIA_RAW_URL}/{citation_commit}");
    [
        VendoredAsset {
            name: "RDKit",
            state_key: "rdkit",
            dir: "rdkit",
            version: rdkit_version,
            files: vec![
                (
                    format!("{rdkit_dist}/RDKit_minimal.js"),
                    "RDKit_minimal.js".to_owned(),
                ),
                (
                    format!("{rdkit_dist}/RDKit_minimal.wasm"),
                    "RDKit_minimal.wasm".to_owned(),
                ),
                (RDKIT_LICENSE_URL.to_owned(), "LICENSE.txt".to_owned()),
            ],
        },
        VendoredAsset {
            name: "Scholia Citation.js",
            state_key: "scholia",
            dir: "citation-js",
            version: citation_commit,
            files: vec![
                (
                    format!("{scholia}/scholia/app/static/js/citation.js"),
                    "citation.js".to_owned(),
                ),
                (
                    format!("{scholia}/LICENSE"),
                    "LICENSE.scholia.txt".to_owned(),
                ),
                (
                    CITATION_LICENSE_URL.to_owned(),
                    "LICENSE.citation-js.txt".to_owned(),
                ),
            ],
        },
    ]
}

/// Parse the state file. One entry per line; unparsable lines are ignored rather
/// than fatal, so a hand-edited file degrades to a re-fetch.
fn parse_state(contents: &str) -> BTreeMap<String, String> {
    contents
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            let key = key.trim();
            if key.is_empty() {
                return None;
            }
            Some((key.to_owned(), value.trim().to_owned()))
        })
        .collect()
}

/// Render the state file, keys sorted so the output is stable.
fn render_state(state: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    for (key, value) in state {
        out.push_str(key);
        out.push('=');
        out.push_str(value);
        out.push('\n');
    }
    out
}

fn write_state(path: &Path, state: &BTreeMap<String, String>) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, render_state(state))
}

/// Fetch `RDKit` and Citation.js unless they are already cached at the current
/// version.
///
/// # Errors
/// Returns the first version-resolution, transport or filesystem failure.
pub fn fetch_curation_assets(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(setting("CURATION_ASSET_DIR", DEFAULT_CURATION_DIR));
    let state_path = PathBuf::from(setting("CURATION_ASSET_STATE", DEFAULT_CURATION_STATE));
    let rdkit_version = resolve_metadata_version(
        client,
        RDKIT_LATEST_METADATA,
        &setting("RDKIT_VERSION", "latest"),
    )?;
    let citation_commit = resolve_github_ref(
        client,
        GITHUB_BASE_URL,
        SCHOLIA_REPO,
        &setting("CITATION_JS_REF", "main"),
    )?;
    let mut state =
        fs::read_to_string(&state_path).map_or_else(|_| BTreeMap::new(), |raw| parse_state(&raw));

    for asset in assets(rdkit_version, citation_commit) {
        asset.refresh(client, &root, &mut state, &state_path)?;
    }
    Ok(())
}
