// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Fetch the external frontend assets used by the web client.
//!
//! Two independent jobs, run in that order:
//!
//! - `vendor` caches `RDKit` and the Citation.js build under
//!   `public/assets/vendor`, keyed on the version each was fetched at;
//! - `ketcher` downloads the structure editor's release zip and lays it out
//!   under `public/assets/ketcher`.
//!
//! `http` holds what both share.
//!
//! The first failure stops the run and is reported, so a partially vendored tree
//! never looks complete: the assets on disk are the ones the last run finished
//! with, and this run says which fetch failed.
//!
//! The modules are `pub` so `unreachable_pub` and
//! `clippy::redundant_pub_crate` agree: in a binary crate they only leave a
//! private module alone if the items inside are reachable from the crate root.

pub mod http;
pub mod ketcher;
#[cfg(test)]
pub mod test_support;
pub mod vendor;

use reqwest::blocking::Client;

/// Fetch everything the web client needs, stopping at the first failure.
fn run(client: &Client, target: &ketcher::KetcherTarget) -> Result<(), String> {
    vendor::fetch_curation_assets(client)
        .map_err(|e| format!("could not vendor the curation assets: {e}"))?;
    ketcher::fetch_ketcher(client, target).map_err(|e| format!("could not fetch Ketcher: {e}"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder().build()?;
    run(&client, &ketcher::KetcherTarget::from_env()).map_err(std::io::Error::other)?;
    Ok(())
}

#[cfg(test)]
mod tests {
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
}
