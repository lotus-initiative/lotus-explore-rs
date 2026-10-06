// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Execution-time metrics for a single search pipeline run.

use crate::services::search_telemetry as telemetry;

/// Wall-clock timings accumulated during a single search execution.
#[derive(Default, Clone, Copy)]
pub struct SearchMetrics {
    /// Total wall time spent waiting for network responses (ms).
    pub network_ms: f64,
    /// Total wall time spent parsing CSV/JSON payloads (ms).
    pub parse_ms: f64,
    /// Number of distinct SPARQL/REST calls issued.
    pub sparql_calls: usize,
}

impl SearchMetrics {
    /// Record a completed sequential network call.
    pub fn add_network(&mut self, elapsed: std::time::Duration) {
        self.network_ms = elapsed.as_secs_f64().mul_add(1000.0, self.network_ms);
        self.sparql_calls += 1;
    }

    /// Record a completed parse phase.
    // Only the native path reports a parse time: it spools the body to a file
    // and then reads it back, while the browser folds chunks as they arrive and
    // has no separate parse phase to time.
    #[cfg(any(not(target_arch = "wasm32"), test))]
    pub fn add_parse(&mut self, elapsed: std::time::Duration) {
        self.parse_ms = elapsed.as_secs_f64().mul_add(1000.0, self.parse_ms);
    }
}

/// Searches taking longer than this threshold are flagged as slow queries in
/// the telemetry log.
const SLOW_QUERY_THRESHOLD_MS: f64 = 5_000.0;

/// Emit a structured summary log line for a completed search.
/// If `total_elapsed` exceeds [`SLOW_QUERY_THRESHOLD_MS`] an additional
/// `slow_query` warning is emitted so dashboards can alert on regressions.
pub fn emit_search_summary(total_elapsed: std::time::Duration, metrics: SearchMetrics) {
    let total_ms = total_elapsed.as_secs_f64() * 1000.0;
    let details = format!(
        "total_ms={total_ms:.1} network_ms={:.1} parse_ms={:.1} sparql_calls={}",
        metrics.network_ms, metrics.parse_ms, metrics.sparql_calls
    );
    telemetry::search_summary_done(&details);
    if total_ms >= SLOW_QUERY_THRESHOLD_MS {
        telemetry::search_summary_slow_query(&details);
    }
}

#[cfg(test)]
#[path = "search_metrics/tests.rs"]
mod tests;
