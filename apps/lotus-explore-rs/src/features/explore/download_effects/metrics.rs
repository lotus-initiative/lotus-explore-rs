// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::app_state::MetricsState;

/// Metrics state after a waiting-for-loading dispatch tick.
/// `logged_waiting_loading` must be `true` only when telemetry was emitted
/// during this tick; this keeps the guard aligned with log side effects.
#[must_use]
pub const fn metrics_for_waiting_loading_phase(
    metrics: &MetricsState,
    logged_waiting_loading: bool,
) -> MetricsState {
    MetricsState {
        waiting_loading_logged: metrics.waiting_loading_logged || logged_waiting_loading,
        waiting_query_logged: false,
    }
}

/// Metrics state after a waiting-for-query dispatch tick.
/// `logged_waiting_query` must be `true` only when telemetry was emitted
/// during this tick; this keeps the guard aligned with log side effects.
#[must_use]
pub const fn metrics_for_waiting_query_phase(
    metrics: &MetricsState,
    logged_waiting_query: bool,
) -> MetricsState {
    MetricsState {
        waiting_loading_logged: false,
        waiting_query_logged: metrics.waiting_query_logged || logged_waiting_query,
    }
}
