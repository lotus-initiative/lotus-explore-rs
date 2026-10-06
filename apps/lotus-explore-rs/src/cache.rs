// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! In-browser result cache.
//!
//! # What this used to hold
//!
//! Up to eight raw CSV response bodies, keyed by query, with whole-window
//! eviction. It was a cache of *payloads*, and it had no bound on payload size —
//! so eight wide searches could sit in memory at eight times the peak response,
//! which is the opposite of what a memory budget needs.
//!
//! What it holds now is the finished [`lotus_model::ColumnarResultSet`], which is the
//! thing the table is built from anyway. Sharing one costs an `Arc` and nothing
//! more, so a cached entry is not a second copy of the result -- and the bound is
//! one, because a cache exists to make the back button free rather than to hold a
//! history.
//!
//! Caching the *set* rather than the body is what makes this possible at all: the
//! body is no longer in memory, so there is nothing else to cache.

#[cfg(any(test, target_arch = "wasm32"))]
#[path = "cache/cache_impl.rs"]
mod cache_impl;

#[cfg(target_arch = "wasm32")]
// `unreachable_pub`-style narrowing requires `pub(crate)` for cross-module
// use; the nursery `redundant_pub_crate` suggestion (`pub`) would widen it.
#[allow(clippy::redundant_pub_crate)]
pub(crate) use cache_impl::{store_cached_set, take_cached_set};
