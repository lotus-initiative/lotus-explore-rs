// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `fetch_assets`, in their own file.

// Test code: a failing assertion is how it reports.
#![allow(clippy::panic, clippy::expect_used)]

use super::*;
use crate::test_support::{MockServer, http_status, temp_dir};

#[test]
fn a_failure_names_the_fetch_that_failed() {
    let dir = temp_dir("fetch-assets-fail");
    let state = dir.join("state");
    // Retryable 503 answers ATTEMPTS times (3) on the local mock server.
    let server = MockServer::start(vec![
        http_status(503, "down"),
        http_status(503, "down"),
        http_status(503, "down"),
    ]);
    let client = Client::builder()
        .timeout(std::time::Duration::from_millis(300))
        .build()
        .unwrap_or_else(|e| panic!("cannot build a client: {e}"));
    let target = ketcher::KetcherTarget {
        dir: dir.join("ketcher"),
        requested: "1.0.0".to_owned(),
        url: Some("http://127.0.0.1:1/none.zip".to_owned()),
    };

    let message = run_with_vendor(
        &client,
        &dir.join("vendor"),
        &state,
        &server.url("/rdkit.json"),
        &server.url("/github"),
        &target,
    )
    .expect_err("mock upstream returns 503")
    .to_string();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        message.starts_with("could not vendor the curation assets"),
        "the message names the failing half, got: {message}"
    );
}
