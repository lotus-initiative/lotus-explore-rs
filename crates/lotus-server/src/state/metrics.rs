// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![expect(
    clippy::redundant_pub_crate,
    reason = "items are re-exported pub(crate) to sibling modules from a private submodule"
)]
//! Atomic runtime counters for `/health` and Prometheus `/metrics`.

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

use crate::types::HealthResponse;

#[derive(Debug)]
pub(crate) struct RuntimeMetrics {
    started_at: Instant,
    pub(crate) search_cache_hits: AtomicU64,
    pub(crate) search_cache_misses: AtomicU64,
    pub(crate) search_inflight_waits: AtomicU64,
    pub(crate) search_upstream_hits: AtomicU64,
    pub(crate) export_cache_hits: AtomicU64,
    pub(crate) export_cache_misses: AtomicU64,
    pub(crate) export_inflight_waits: AtomicU64,
    pub(crate) export_upstream_hits: AtomicU64,
    pub(crate) overload_rejections: AtomicU64,
    /// Searches refused because every `QLever` slot was busy.
    pub(crate) upstream_shed: AtomicU64,
    pub(crate) request_timeouts: AtomicU64,
}

impl RuntimeMetrics {
    pub(crate) fn new() -> Self {
        Self {
            started_at: Instant::now(),
            search_cache_hits: AtomicU64::new(0),
            search_cache_misses: AtomicU64::new(0),
            search_inflight_waits: AtomicU64::new(0),
            search_upstream_hits: AtomicU64::new(0),
            export_cache_hits: AtomicU64::new(0),
            export_cache_misses: AtomicU64::new(0),
            export_inflight_waits: AtomicU64::new(0),
            export_upstream_hits: AtomicU64::new(0),
            overload_rejections: AtomicU64::new(0),
            upstream_shed: AtomicU64::new(0),
            request_timeouts: AtomicU64::new(0),
        }
    }

    pub(crate) fn snapshot(&self) -> HealthResponse {
        HealthResponse {
            status: "ok",
            uptime_secs: self.started_at.elapsed().as_secs(),
            search_cache_hits: self.search_cache_hits.load(Ordering::Relaxed),
            search_cache_misses: self.search_cache_misses.load(Ordering::Relaxed),
            search_inflight_waits: self.search_inflight_waits.load(Ordering::Relaxed),
            search_upstream_hits: self.search_upstream_hits.load(Ordering::Relaxed),
            export_cache_hits: self.export_cache_hits.load(Ordering::Relaxed),
            export_cache_misses: self.export_cache_misses.load(Ordering::Relaxed),
            export_inflight_waits: self.export_inflight_waits.load(Ordering::Relaxed),
            export_upstream_hits: self.export_upstream_hits.load(Ordering::Relaxed),
            overload_rejections: self.overload_rejections.load(Ordering::Relaxed),
            upstream_shed: self.upstream_shed.load(Ordering::Relaxed),
            request_timeouts: self.request_timeouts.load(Ordering::Relaxed),
        }
    }

    pub(crate) fn render_prometheus(&self) -> String {
        let snapshot = self.snapshot();
        format!(
            concat!(
                "# HELP lotus_api_health_status Health status indicator.\n",
                "# TYPE lotus_api_health_status gauge\n",
                "lotus_api_health_status{{status=\"{}\"}} 1\n",
                "# HELP lotus_api_uptime_seconds Time since process start.\n",
                "# TYPE lotus_api_uptime_seconds gauge\n",
                "lotus_api_uptime_seconds {}\n",
                "# HELP lotus_api_search_cache_hits Total search cache hits.\n",
                "# TYPE lotus_api_search_cache_hits counter\n",
                "lotus_api_search_cache_hits {}\n",
                "# HELP lotus_api_search_cache_misses Total search cache misses.\n",
                "# TYPE lotus_api_search_cache_misses counter\n",
                "lotus_api_search_cache_misses {}\n",
                "# HELP lotus_api_search_inflight_waits Search requests coalesced behind an in-flight request.\n",
                "# TYPE lotus_api_search_inflight_waits counter\n",
                "lotus_api_search_inflight_waits {}\n",
                "# HELP lotus_api_search_upstream_hits Search requests that reached upstream execution.\n",
                "# TYPE lotus_api_search_upstream_hits counter\n",
                "lotus_api_search_upstream_hits {}\n",
                "# HELP lotus_api_export_cache_hits Total export cache hits.\n",
                "# TYPE lotus_api_export_cache_hits counter\n",
                "lotus_api_export_cache_hits {}\n",
                "# HELP lotus_api_export_cache_misses Total export cache misses.\n",
                "# TYPE lotus_api_export_cache_misses counter\n",
                "lotus_api_export_cache_misses {}\n",
                "# HELP lotus_api_export_inflight_waits Export requests coalesced behind an in-flight request.\n",
                "# TYPE lotus_api_export_inflight_waits counter\n",
                "lotus_api_export_inflight_waits {}\n",
                "# HELP lotus_api_export_upstream_hits Export requests that reached upstream execution.\n",
                "# TYPE lotus_api_export_upstream_hits counter\n",
                "lotus_api_export_upstream_hits {}\n",
                "# HELP lotus_api_overload_rejections Requests rejected because the server was busy.\n",
                "# TYPE lotus_api_overload_rejections counter\n",
                "lotus_api_overload_rejections {}\n",
                "# HELP lotus_api_upstream_shed Searches refused because every upstream query slot was busy.\n",
                "# TYPE lotus_api_upstream_shed counter\n",
                "lotus_api_upstream_shed {}\n",
                "# HELP lotus_api_request_timeouts Requests that exceeded the configured timeout.\n",
                "# TYPE lotus_api_request_timeouts counter\n",
                "lotus_api_request_timeouts {}\n"
            ),
            snapshot.status,
            snapshot.uptime_secs,
            snapshot.search_cache_hits,
            snapshot.search_cache_misses,
            snapshot.search_inflight_waits,
            snapshot.search_upstream_hits,
            snapshot.export_cache_hits,
            snapshot.export_cache_misses,
            snapshot.export_inflight_waits,
            snapshot.export_upstream_hits,
            snapshot.overload_rejections,
            snapshot.upstream_shed,
            snapshot.request_timeouts,
        )
    }
}
