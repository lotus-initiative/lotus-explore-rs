// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Shared server runtime state: admission/upstream semaphores, bounded TTL caches,
//! in-flight request coalescing cells, and observability counters.

mod cache;
mod inflight;
mod metrics;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use tokio::sync::{OnceCell, Semaphore};

use crate::{
    config::AppConfig,
    dto::{ExportUrlResponse, SearchResponse},
    errors::SharedApiError,
};

pub use self::cache::{
    CachedExportResponse, CachedTaxonResolution, export_cache_get, export_cache_put,
    search_cache_get, search_cache_put, taxon_cache_get, taxon_cache_put,
};
#[cfg(test)]
pub use self::cache::{CachedSearchResponse, maybe_prune_cache_at, prune_cache};
pub use self::inflight::{
    export_inflight_cell, export_inflight_remove, search_inflight_cell, search_inflight_remove,
};
pub use self::metrics::RuntimeMetrics;

pub const TAXON_CACHE_TTL: Duration = Duration::from_hours(24);
pub const SEARCH_CACHE_TTL: Duration = Duration::from_mins(3);
pub const EXPORT_CACHE_TTL: Duration = Duration::from_mins(10);
pub const CACHE_PRUNE_INTERVAL: Duration = Duration::from_secs(20);
pub const MAX_TAXON_CACHE_ENTRIES: usize = 512;
pub const MAX_SEARCH_CACHE_ENTRIES: usize = 128;
pub const MAX_EXPORT_CACHE_ENTRIES: usize = 256;

pub type InFlightSearch = Arc<OnceCell<Result<SearchResponse, SharedApiError>>>;
pub type InFlightExport = Arc<OnceCell<Result<ExportUrlResponse, SharedApiError>>>;

/// Shared state cloned across Axum request handlers.
#[derive(Clone, Debug)]
pub struct AppState {
    pub default_limit: usize,
    pub request_timeout: Duration,
    /// How long an upstream query waits for a permit before the search is shed.
    pub upstream_queue_wait: Duration,
    pub request_permits: Arc<Semaphore>,
    /// Polite upstream budget for concurrent queries to `QLever`.
    pub upstream_permits: Arc<Semaphore>,
    pub taxon_cache: Arc<Mutex<HashMap<String, CachedTaxonResolution>>>,
    pub search_cache: Arc<Mutex<HashMap<String, self::cache::CachedSearchResponse>>>,
    pub export_cache: Arc<Mutex<HashMap<String, CachedExportResponse>>>,
    pub search_inflight: Arc<Mutex<HashMap<String, InFlightSearch>>>,
    pub export_inflight: Arc<Mutex<HashMap<String, InFlightExport>>>,
    pub taxon_cache_prune_after: Arc<Mutex<Instant>>,
    pub search_cache_prune_after: Arc<Mutex<Instant>>,
    pub export_cache_prune_after: Arc<Mutex<Instant>>,
    pub metrics: Arc<RuntimeMetrics>,
}

impl AppState {
    /// Construct initial server state sized from `config`.
    #[must_use]
    pub fn new(config: &AppConfig) -> Self {
        let prune_at = Instant::now() + CACHE_PRUNE_INTERVAL;
        Self {
            default_limit: config.default_limit,
            request_timeout: config.request_timeout,
            upstream_queue_wait: config.upstream_queue_wait,
            request_permits: Arc::new(Semaphore::new(config.max_concurrency)),
            upstream_permits: Arc::new(Semaphore::new(config.upstream_concurrency)),
            taxon_cache: Arc::new(Mutex::new(HashMap::new())),
            search_cache: Arc::new(Mutex::new(HashMap::new())),
            export_cache: Arc::new(Mutex::new(HashMap::new())),
            search_inflight: Arc::new(Mutex::new(HashMap::new())),
            export_inflight: Arc::new(Mutex::new(HashMap::new())),
            taxon_cache_prune_after: Arc::new(Mutex::new(prune_at)),
            search_cache_prune_after: Arc::new(Mutex::new(prune_at)),
            export_cache_prune_after: Arc::new(Mutex::new(prune_at)),
            metrics: Arc::new(RuntimeMetrics::new()),
        }
    }
}
