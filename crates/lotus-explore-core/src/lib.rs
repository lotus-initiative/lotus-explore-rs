// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

//! The LOTUS Explorer's domain logic.
//!
//! The modules are public rather than flattened into the crate root, because
//! the names repeat: `explore::request::SearchRequest` and `api::dto::SearchRequest`
//! are different things, and a flat root would have to invent a prefix for one
//! of them.

/// The wire format the REST API speaks.
pub mod api;
/// The explore feature domain.
pub mod explore;
/// What an export looks like on disk.
pub mod export;
/// Client-side narrowing of an already-fetched result set.
pub mod filters;
/// The transport boundary: what the domain asks a host to do.
pub mod ports;
/// How the results table is ordered.
pub mod sort;
/// How many rows one request may ask for.
pub mod table_budget;
