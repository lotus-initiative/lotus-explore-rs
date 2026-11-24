// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The REST client and its configuration.
//!
//! The wire format itself lives in `lotus-explore-core`: the request and
//! response shapes are the contract, and the contract does not change when the
//! host does. What is left here is the part that performs I/O.

pub mod client;
pub mod config;
