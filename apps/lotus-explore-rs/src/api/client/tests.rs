// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `client`, in their own file.

#[cfg(target_arch = "wasm32")]
use super::*;

#[cfg(target_arch = "wasm32")]
#[test]
fn resolve_api_url_joins_relative_paths() {
    assert_eq!(
        resolve_api_url("https://api.example.org", "/v1/export-file/abc/csv"),
        "https://api.example.org/v1/export-file/abc/csv"
    );
    assert_eq!(
        resolve_api_url("https://api.example.org/", "v1/export-file/abc/csv"),
        "https://api.example.org/v1/export-file/abc/csv"
    );
}
