// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![expect(
    clippy::redundant_pub_crate,
    reason = "items are re-exported pub(crate) to sibling modules from a private submodule"
)]
//! Bounded, TTL-pruned in-memory caches for taxon, search, and export responses.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use super::{
    AppState, CACHE_PRUNE_INTERVAL, EXPORT_CACHE_TTL, MAX_EXPORT_CACHE_ENTRIES,
    MAX_SEARCH_CACHE_ENTRIES, MAX_TAXON_CACHE_ENTRIES, SEARCH_CACHE_TTL, TAXON_CACHE_TTL,
};
use crate::types::{ExportUrlResponse, SearchResponse};

#[derive(Clone, Debug)]
pub(crate) struct CachedTaxonResolution {
    pub(crate) inserted_at: Instant,
    pub(crate) value: (Option<String>, Option<String>),
}

#[derive(Clone, Debug)]
pub(crate) struct CachedSearchResponse {
    pub(crate) inserted_at: Instant,
    pub(crate) value: SearchResponse,
}

#[derive(Clone, Debug)]
pub(crate) struct CachedExportResponse {
    pub(crate) inserted_at: Instant,
    pub(crate) value: ExportUrlResponse,
}

pub(crate) fn search_cache_get(state: &AppState, key: &str) -> Option<SearchResponse> {
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

pub(crate) fn search_cache_put(state: &AppState, key: String, value: SearchResponse) {
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

pub(crate) fn export_cache_get(state: &AppState, key: &str) -> Option<ExportUrlResponse> {
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

pub(crate) fn export_cache_put(state: &AppState, key: String, value: ExportUrlResponse) {
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

pub(crate) fn taxon_cache_get(
    state: &AppState,
    key: &str,
) -> Option<(Option<String>, Option<String>)> {
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

pub(crate) fn taxon_cache_put(
    state: &AppState,
    key: String,
    value: (Option<String>, Option<String>),
) {
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
    maybe_prune_cache_at(
        cache,
        prune_after,
        ttl,
        max_entries,
        Instant::now(),
        inserted_at,
    );
}

pub(crate) fn maybe_prune_cache_at<V, F>(
    cache: &mut HashMap<String, V>,
    prune_after: &Arc<Mutex<Instant>>,
    ttl: Duration,
    max_entries: usize,
    now: Instant,
    inserted_at: F,
) where
    F: Fn(&V) -> Instant,
{
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

pub(crate) fn prune_cache<V, F>(
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
