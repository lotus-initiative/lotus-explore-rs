// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! In-browser result cache.
//!
//! One entry, keyed by query, holding the finished [`lotus_model::ColumnarResultSet`]
//! the table is built from anyway. Sharing one costs an `Arc` and nothing more, so
//! a cached entry is not a second copy of the result, and the body is not in
//! memory to cache. One because a cache here exists to make the back button free,
//! not to hold a history.

#[cfg(any(test, target_arch = "wasm32"))]
#[path = "cache/cache_impl.rs"]
mod cache_impl;

#[cfg(target_arch = "wasm32")]
// `unreachable_pub`-style narrowing requires `pub(crate)` for cross-module
// use; the nursery `redundant_pub_crate` suggestion (`pub`) would widen it.
#[allow(clippy::redundant_pub_crate)]
pub(crate) use cache_impl::{store_cached_set, take_cached_set};
