// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![allow(
    unused_crate_dependencies,
    reason = "binary entrypoint delegates directly to the lotus-server library crate"
)]
//! Binary entrypoint for `lotus-server`.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    lotus_server::run().await
}
