// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Shared helpers for the repository checks: where the repository is, and
//! reading a file from it.
//!
//! Everything here returns `Result` rather than panicking, because the workspace
//! denies `clippy::unwrap_used` and `clippy::expect_used` and that reaches test
//! targets.

use std::error::Error;
use std::path::PathBuf;

pub(super) type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub(super) fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub(super) fn read(path: &str) -> Result<String> {
    let full = repo_root().join(path);
    std::fs::read_to_string(&full).map_err(|e| format!("{}: {e}", full.display()).into())
}
