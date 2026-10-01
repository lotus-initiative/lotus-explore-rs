// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

pub use crate::cache_key::{build_export_cache_key, build_search_cache_key};
use crate::server::{
    config::AppConfig,
    errors::ApiError,
    types::{ExportUrlResponse, HealthResponse, SearchResponse},
};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{OnceCell, Semaphore};

pub const TAXON_CACHE_TTL: Duration = Duration::from_hours(24);
pub const SEARCH_CACHE_TTL: Duration = Duration::from_mins(3);
pub const EXPORT_CACHE_TTL: Duration = Duration::from_mins(10);
const CACHE_PRUNE_INTERVAL: Duration = Duration::from_secs(20);
const MAX_TAXON_CACHE_ENTRIES: usize = 512;
const MAX_SEARCH_CACHE_ENTRIES: usize = 128;
const MAX_EXPORT_CACHE_ENTRIES: usize = 256;

pub type InFlightSearch =
    Arc<OnceCell<Result<SearchResponse, crate::server::errors::SharedApiError>>>;
pub type InFlightExport =
    Arc<OnceCell<Result<ExportUrlResponse, crate::server::errors::SharedApiError>>>;

#[derive(Clone)]
pub struct AppState {
    pub(crate) default_limit: usize,
    pub(crate) request_timeout: Duration,
    pub(crate) request_permits: Arc<Semaphore>,
    pub(crate) taxon_cache: Arc<Mutex<HashMap<String, CachedTaxonResolution>>>,
    pub(crate) search_cache: Arc<Mutex<HashMap<String, CachedSearchResponse>>>,
    pub(crate) export_cache: Arc<Mutex<HashMap<String, CachedExportResponse>>>,
    pub(crate) search_inflight: Arc<Mutex<HashMap<String, InFlightSearch>>>,
    pub(crate) export_inflight: Arc<Mutex<HashMap<String, InFlightExport>>>,
    pub(crate) taxon_cache_prune_after: Arc<Mutex<Instant>>,
    pub(crate) search_cache_prune_after: Arc<Mutex<Instant>>,
    pub(crate) export_cache_prune_after: Arc<Mutex<Instant>>,
    pub(crate) metrics: Arc<RuntimeMetrics>,
}

impl AppState {
    pub(crate) fn new(config: &AppConfig) -> Self {
        Self {
            default_limit: config.default_limit,
            request_timeout: config.request_timeout,
            request_permits: Arc::new(Semaphore::new(config.max_concurrency)),
            taxon_cache: Arc::new(Mutex::new(HashMap::new())),
            search_cache: Arc::new(Mutex::new(HashMap::new())),
            export_cache: Arc::new(Mutex::new(HashMap::new())),
            search_inflight: Arc::new(Mutex::new(HashMap::new())),
            export_inflight: Arc::new(Mutex::new(HashMap::new())),
            taxon_cache_prune_after: Arc::new(Mutex::new(Instant::now() + CACHE_PRUNE_INTERVAL)),
            search_cache_prune_after: Arc::new(Mutex::new(Instant::now() + CACHE_PRUNE_INTERVAL)),
            export_cache_prune_after: Arc::new(Mutex::new(Instant::now() + CACHE_PRUNE_INTERVAL)),
            metrics: Arc::new(RuntimeMetrics::new()),
        }
    }
}

#[derive(Clone)]
pub struct CachedTaxonResolution {
    pub(crate) inserted_at: Instant,
    pub(crate) value: (Option<String>, Option<String>),
}

#[derive(Clone)]
pub struct CachedSearchResponse {
    pub(crate) inserted_at: Instant,
    pub(crate) value: SearchResponse,
}

#[derive(Clone)]
pub struct CachedExportResponse {
    pub(crate) inserted_at: Instant,
    pub(crate) value: ExportUrlResponse,
}

pub struct RuntimeMetrics {
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
            snapshot.request_timeouts,
        )
    }
}

pub fn search_cache_get(state: &AppState, key: &str) -> Option<SearchResponse> {
    let mut cache = state.search_cache.lock().ok()?;
    maybe_prune_cache(
        &mut cache,
        &state.search_cache_prune_after,
        SEARCH_CACHE_TTL,
        MAX_SEARCH_CACHE_ENTRIES,
        |entry| entry.inserted_at,
    );
    cache.get(key).map(|entry| entry.value.clone())
}

pub fn search_cache_put(state: &AppState, key: String, value: SearchResponse) {
    if let Ok(mut cache) = state.search_cache.lock() {
        maybe_prune_cache(
            &mut cache,
            &state.search_cache_prune_after,
            SEARCH_CACHE_TTL,
            MAX_SEARCH_CACHE_ENTRIES,
            |entry| entry.inserted_at,
        );
        cache.insert(
            key,
            CachedSearchResponse {
                inserted_at: Instant::now(),
                value,
            },
        );
    }
}

pub fn export_cache_get(state: &AppState, key: &str) -> Option<ExportUrlResponse> {
    let mut cache = state.export_cache.lock().ok()?;
    maybe_prune_cache(
        &mut cache,
        &state.export_cache_prune_after,
        EXPORT_CACHE_TTL,
        MAX_EXPORT_CACHE_ENTRIES,
        |entry| entry.inserted_at,
    );
    cache.get(key).map(|entry| entry.value.clone())
}

pub fn export_cache_put(state: &AppState, key: String, value: ExportUrlResponse) {
    if let Ok(mut cache) = state.export_cache.lock() {
        maybe_prune_cache(
            &mut cache,
            &state.export_cache_prune_after,
            EXPORT_CACHE_TTL,
            MAX_EXPORT_CACHE_ENTRIES,
            |entry| entry.inserted_at,
        );
        cache.insert(
            key,
            CachedExportResponse {
                inserted_at: Instant::now(),
                value,
            },
        );
    }
}

pub fn search_inflight_cell(
    state: &AppState,
    key: &str,
) -> Result<(InFlightSearch, bool), ApiError> {
    // `Mutex::lock` errors only on poison -- a holder thread panicked while
    // holding the guard. That is converted to a 500 rather than unwrapped,
    // because unwrapping aborts the whole process: one poisoned lock would take
    // down every other in-flight request with it.
    let existing = state
        .search_inflight
        .lock()
        .map_err(|_| ApiError::internal("search inflight mutex poisoned"))?
        .get(key)
        .cloned();
    if let Some(existing) = existing {
        return Ok((existing, false));
    }
    let cell = Arc::new(OnceCell::new());
    state
        .search_inflight
        .lock()
        .map_err(|_| ApiError::internal("search inflight mutex poisoned"))?
        .insert(key.to_string(), cell.clone());
    Ok((cell, true))
}

pub fn search_inflight_remove(state: &AppState, key: &str, cell: &InFlightSearch, is_leader: bool) {
    if !is_leader {
        return;
    }
    if let Ok(mut inflight) = state.search_inflight.lock()
        && inflight
            .get(key)
            .is_some_and(|current| Arc::ptr_eq(current, cell))
    {
        inflight.remove(key);
    }
}

pub fn export_inflight_cell(
    state: &AppState,
    key: &str,
) -> Result<(InFlightExport, bool), ApiError> {
    // As in `search_inflight_cell`: a poisoned lock becomes a typed error.
    let existing = state
        .export_inflight
        .lock()
        .map_err(|_| ApiError::internal("export inflight mutex poisoned"))?
        .get(key)
        .cloned();
    if let Some(existing) = existing {
        return Ok((existing, false));
    }
    let cell = Arc::new(OnceCell::new());
    state
        .export_inflight
        .lock()
        .map_err(|_| ApiError::internal("export inflight mutex poisoned"))?
        .insert(key.to_string(), cell.clone());
    Ok((cell, true))
}

pub fn export_inflight_remove(state: &AppState, key: &str, cell: &InFlightExport, is_leader: bool) {
    if !is_leader {
        return;
    }
    if let Ok(mut inflight) = state.export_inflight.lock()
        && inflight
            .get(key)
            .is_some_and(|current| Arc::ptr_eq(current, cell))
    {
        inflight.remove(key);
    }
}

pub fn taxon_cache_get(state: &AppState, key: &str) -> Option<(Option<String>, Option<String>)> {
    let mut cache = state.taxon_cache.lock().ok()?;
    maybe_prune_cache(
        &mut cache,
        &state.taxon_cache_prune_after,
        TAXON_CACHE_TTL,
        MAX_TAXON_CACHE_ENTRIES,
        |entry| entry.inserted_at,
    );
    cache.get(key).map(|entry| entry.value.clone())
}

pub fn taxon_cache_put(state: &AppState, key: String, value: (Option<String>, Option<String>)) {
    if let Ok(mut cache) = state.taxon_cache.lock() {
        maybe_prune_cache(
            &mut cache,
            &state.taxon_cache_prune_after,
            TAXON_CACHE_TTL,
            MAX_TAXON_CACHE_ENTRIES,
            |entry| entry.inserted_at,
        );
        cache.insert(
            key,
            CachedTaxonResolution {
                inserted_at: Instant::now(),
                value,
            },
        );
    }
}

fn maybe_prune_cache<V, F>(
    cache: &mut HashMap<String, V>,
    prune_after: &Arc<Mutex<Instant>>,
    ttl: Duration,
    max_entries: usize,
    inserted_at: F,
) where
    F: Fn(&V) -> Instant,
{
    let now = Instant::now();
    let should_prune = prune_after.lock().map_or(true, |mut next_prune| {
        if now < *next_prune {
            false
        } else {
            *next_prune = now + CACHE_PRUNE_INTERVAL;
            true
        }
    });

    if should_prune {
        prune_cache(cache, ttl, max_entries, inserted_at);
    }
}

pub fn prune_cache<V, F>(
    cache: &mut HashMap<String, V>,
    ttl: Duration,
    max_entries: usize,
    inserted_at: F,
) where
    F: Fn(&V) -> Instant,
{
    let now = Instant::now();
    cache.retain(|_, value| now.duration_since(inserted_at(value)) <= ttl);
    while cache.len() > max_entries {
        let Some(oldest_key) = cache
            .iter()
            .min_by_key(|(_, value)| inserted_at(value))
            .map(|(key, _)| key.clone())
        else {
            break;
        };
        cache.remove(&oldest_key);
    }
}
