// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Fetch the external frontend assets used by the web client.
//!
//! Two independent jobs, in that order, because they fail independently:
//!
//! - `vendor` caches `RDKit` and the Citation.js build under
//!   `public/assets/vendor`, keyed on the version each was fetched at;
//! - `ketcher` downloads the structure editor's release zip and lays it out
//!   under `public/assets/ketcher`.
//!
//! `http` holds what both share. Neither job is required for the other, so a
//! failure in one still leaves the other's assets on disk.
//!
//! The modules are `pub` so that `unreachable_pub` and `clippy::redundant_pub_crate`
//! agree: in a binary crate they only leave a private module alone if the items
//! inside it are reachable from the crate root.

pub mod http;
pub mod ketcher;
#[cfg(test)]
pub mod test_support;
pub mod vendor;

use reqwest::blocking::Client;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder().build()?;
    vendor::fetch_curation_assets(&client)?;
    ketcher::fetch_ketcher(&client, &ketcher::KetcherTarget::from_env())
}
