// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Adapter state and dispatch for the Explore feature.
//!
//! The pure state and reducer live in `lotus-explore-core`; this module only
//! wraps them around a Dioxus signal.

mod dispatch;
pub use dispatch::dispatch_explore_action;
pub use lotus_explore_core::explore::search_state::{ExploreState, reduce_mut};
