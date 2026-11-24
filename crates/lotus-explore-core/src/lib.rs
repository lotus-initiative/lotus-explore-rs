// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![doc = include_str!("../README.md")]
// `ports::LotusRepository` exposes `async fn` returning `!Send` futures on
// wasm32 because the browser transport and UI signals hold `Rc`/`RefCell`.
#![cfg_attr(
    target_arch = "wasm32",
    allow(
        clippy::future_not_send,
        reason = "browser HTTP futures hold Rc and run on one thread"
    )
)]

//! The LOTUS Explorer's domain logic.
//!
//! The modules are public rather than flattened into the crate root, because
//! the names repeat: `explore::request::SearchRequest` (the client-side search
//! envelope) and `api::dto::SearchRequest` (the REST wire payload) are different
//! things, and a flat root would have to invent a prefix for one of them.

/// The wire format the REST API speaks.
pub mod api;
/// The explore feature domain.
pub mod explore;
/// What an export looks like on disk.
pub mod export;
/// Client-side narrowing of an already-fetched result set.
pub mod filters;
/// Network graph projection over a columnar result set.
pub mod graph;
/// The transport boundary: what the domain asks a host to do.
pub mod ports;
/// How the results table is ordered.
pub mod sort;
/// How many rows one request may ask for.
pub mod table_budget;
