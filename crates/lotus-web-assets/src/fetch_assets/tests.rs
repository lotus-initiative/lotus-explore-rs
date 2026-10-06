// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `fetch_assets`, in their own file.

// Test code: a failing assertion is how it reports.
#![allow(clippy::panic, clippy::expect_used)]

use super::*;

#[test]
fn a_failure_names_the_fetch_that_failed() {
    // `example.invalid` never resolves, so the first job fails. The message
    // has to say which one, or a developer reading a failed `fetch-assets`
    // cannot tell which half of the vendor tree is stale.
    let client = Client::builder()
        .timeout(std::time::Duration::from_millis(300))
        .build()
        .unwrap_or_else(|e| panic!("cannot build a client: {e}"));
    let target = ketcher::KetcherTarget {
        dir: std::env::temp_dir(),
        requested: "1.0.0".to_owned(),
        url: Some("http://127.0.0.1:1/none.zip".to_owned()),
    };

    let message = run(&client, &target).expect_err("the vendored host does not resolve");

    assert!(
        message.starts_with("could not vendor the curation assets"),
        "the message names the failing half, got: {message}"
    );
}
