// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `strategy`, in their own file.

use super::*;

#[test]
fn download_flag_selects_download_only() {
    assert_eq!(
        ExecutionStrategy::resolve(true, /* api_enabled */ false),
        ExecutionStrategy::DownloadOnly
    );
}

#[test]
fn normal_flag_selects_direct_by_default() {
    // The API fast-path is opt-in: with no API configured, interactive
    // searches go direct to SPARQL rather than hitting the REST API.
    assert_eq!(
        ExecutionStrategy::resolve(false, /* api_enabled */ false),
        ExecutionStrategy::Direct
    );
}

#[test]
fn api_enabled_flag_selects_api_first() {
    assert_eq!(
        ExecutionStrategy::resolve(false, /* api_enabled */ true),
        ExecutionStrategy::ApiFirst
    );
}

#[test]
fn download_only_is_download_only() {
    assert!(ExecutionStrategy::DownloadOnly.is_download_only());
    assert!(!ExecutionStrategy::ApiFirst.is_download_only());
    assert!(!ExecutionStrategy::Direct.is_download_only());
}
