// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Zenodo REST API v1 request/response DTOs and upload validation.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::bail;
use serde::{Deserialize, Serialize};

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
pub(super) struct Deposition {
    pub(super) id: u64,
    #[serde(default)]
    pub(super) submitted: bool,
    #[serde(default)]
    pub(super) record_id: Option<u64>,
    #[serde(default)]
    pub(super) files: Vec<DepositionFile>,
    #[serde(default)]
    pub(super) links: DepositionLinks,
}

#[derive(Debug, Deserialize)]
pub(super) struct DepositionFile {
    pub(super) id: String,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct DepositionLinks {
    #[serde(rename = "self", default)]
    pub(super) self_: Option<String>,
    #[serde(default)]
    pub(super) bucket: Option<String>,
    #[serde(default)]
    pub(super) latest_draft: Option<String>,
    #[serde(default)]
    pub(super) latest: Option<String>,
}
