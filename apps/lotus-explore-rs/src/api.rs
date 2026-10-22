// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The REST client and its configuration.
//!
//! The wire format itself lives in `lotus-explore-core`: the request and
//! response shapes are the contract, and the contract does not change when the
//! host does. What is left here is the part that performs I/O.

mod client;
mod config;

#[cfg(target_arch = "wasm32")]
pub use client::export_urls;
pub use client::search;
pub use config::api_base_url;
#[cfg(target_arch = "wasm32")]
pub use lotus_explore_core::api::dto::ExportUrlResponse;
pub use lotus_explore_core::api::dto::SearchResponse;
