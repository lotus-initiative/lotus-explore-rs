// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Pure effect scheduling helpers and lifecycle state queries for download orchestration.
//! This module separates concerns from the hook implementations in `download_dispatch.rs`:
//! More detail in the type and function docs below.

mod dispatch;
mod metrics;
mod startup;

pub use dispatch::{DispatchPhase, classify_dispatch_phase};
pub use metrics::{metrics_for_waiting_loading_phase, metrics_for_waiting_query_phase};
pub use startup::{StartupTriggerMode, should_trigger_startup_search};

#[cfg(test)]
mod tests;
