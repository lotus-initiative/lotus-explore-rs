// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
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
use crate::dto::{ExportUrlResponse, SearchResponse};

#[derive(Clone, Debug)]
pub struct CachedTaxonResolution {
    pub inserted_at: Instant,
    pub value: (Option<String>, Option<String>),
}

#[derive(Clone, Debug)]
pub struct CachedSearchResponse {
    pub inserted_at: Instant,
    pub value: SearchResponse,
}

#[derive(Clone, Debug)]
pub struct CachedExportResponse {
    pub inserted_at: Instant,
    pub value: ExportUrlResponse,
}

pub fn search_cache_get(state: &AppState, key: &str) -> Option<SearchResponse> {
    let mut cache = state.search_cache.lock().ok()?;
    let now = Instant::now();
    maybe_prune_cache_at(
        &mut cache,
        &state.search_cache_prune_after,
        SEARCH_CACHE_TTL,
        MAX_SEARCH_CACHE_ENTRIES,
        now,
        |entry| entry.inserted_at,
    );
    get_fresh(&mut cache, key, SEARCH_CACHE_TTL, now, |e| e.inserted_at).map(|entry| entry.value)
}

pub fn search_cache_put(state: &AppState, key: String, value: SearchResponse) {
    if let Ok(mut cache) = state.search_cache.lock() {
        let now = Instant::now();
        maybe_prune_cache_at(
            &mut cache,
            &state.search_cache_prune_after,
            SEARCH_CACHE_TTL,
            MAX_SEARCH_CACHE_ENTRIES,
            now,
            |entry| entry.inserted_at,
        );
        ensure_capacity(
            &mut cache,
            &key,
            SEARCH_CACHE_TTL,
            MAX_SEARCH_CACHE_ENTRIES,
            now,
            |entry| entry.inserted_at,
        );
        cache.insert(
            key,
            CachedSearchResponse {
                inserted_at: now,
                value,
            },
        );
    }
}

pub fn export_cache_get(state: &AppState, key: &str) -> Option<ExportUrlResponse> {
    let mut cache = state.export_cache.lock().ok()?;
    let now = Instant::now();
    maybe_prune_cache_at(
        &mut cache,
        &state.export_cache_prune_after,
        EXPORT_CACHE_TTL,
        MAX_EXPORT_CACHE_ENTRIES,
        now,
        |entry| entry.inserted_at,
    );
    get_fresh(&mut cache, key, EXPORT_CACHE_TTL, now, |e| e.inserted_at).map(|entry| entry.value)
}

pub fn export_cache_put(state: &AppState, key: String, value: ExportUrlResponse) {
    if let Ok(mut cache) = state.export_cache.lock() {
        let now = Instant::now();
        maybe_prune_cache_at(
            &mut cache,
            &state.export_cache_prune_after,
            EXPORT_CACHE_TTL,
            MAX_EXPORT_CACHE_ENTRIES,
            now,
            |entry| entry.inserted_at,
        );
        ensure_capacity(
            &mut cache,
            &key,
            EXPORT_CACHE_TTL,
            MAX_EXPORT_CACHE_ENTRIES,
            now,
            |entry| entry.inserted_at,
        );
        cache.insert(
            key,
            CachedExportResponse {
                inserted_at: now,
                value,
            },
        );
    }
}

pub fn taxon_cache_get(state: &AppState, key: &str) -> Option<(Option<String>, Option<String>)> {
    let mut cache = state.taxon_cache.lock().ok()?;
    let now = Instant::now();
    maybe_prune_cache_at(
        &mut cache,
        &state.taxon_cache_prune_after,
        TAXON_CACHE_TTL,
        MAX_TAXON_CACHE_ENTRIES,
        now,
        |entry| entry.inserted_at,
    );
    get_fresh(&mut cache, key, TAXON_CACHE_TTL, now, |e| e.inserted_at).map(|entry| entry.value)
}

pub fn taxon_cache_put(state: &AppState, key: String, value: (Option<String>, Option<String>)) {
    if let Ok(mut cache) = state.taxon_cache.lock() {
        let now = Instant::now();
        maybe_prune_cache_at(
            &mut cache,
            &state.taxon_cache_prune_after,
            TAXON_CACHE_TTL,
            MAX_TAXON_CACHE_ENTRIES,
            now,
            |entry| entry.inserted_at,
        );
        ensure_capacity(
            &mut cache,
            &key,
            TAXON_CACHE_TTL,
            MAX_TAXON_CACHE_ENTRIES,
            now,
            |entry| entry.inserted_at,
        );
        cache.insert(
            key,
            CachedTaxonResolution {
                inserted_at: now,
                value,
            },
        );
    }
}

fn get_fresh<V: Clone, F>(
    cache: &mut HashMap<String, V>,
    key: &str,
    ttl: Duration,
    now: Instant,
    inserted_at: F,
) -> Option<V>
where
    F: Fn(&V) -> Instant,
{
    let is_fresh = cache
        .get(key)
        .is_some_and(|entry| now.saturating_duration_since(inserted_at(entry)) <= ttl);
    if is_fresh {
        cache.get(key).cloned()
    } else {
        cache.remove(key);
        None
    }
}

fn ensure_capacity<V, F>(
    cache: &mut HashMap<String, V>,
    incoming_key: &str,
    ttl: Duration,
    max_entries: usize,
    now: Instant,
    inserted_at: F,
) where
    F: Fn(&V) -> Instant,
{
    if cache.contains_key(incoming_key) || cache.len() < max_entries {
        return;
    }
    prune_cache_at(cache, ttl, max_entries.saturating_sub(1), now, inserted_at);
}

pub fn maybe_prune_cache_at<V, F>(
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
        prune_cache_at(cache, ttl, max_entries, now, inserted_at);
    }
}

#[cfg(test)]
pub fn prune_cache<V, F>(
    cache: &mut HashMap<String, V>,
    ttl: Duration,
    max_entries: usize,
    inserted_at: F,
) where
    F: Fn(&V) -> Instant,
{
    prune_cache_at(cache, ttl, max_entries, Instant::now(), inserted_at);
}

fn prune_cache_at<V, F>(
    cache: &mut HashMap<String, V>,
    ttl: Duration,
    max_entries: usize,
    now: Instant,
    inserted_at: F,
) where
    F: Fn(&V) -> Instant,
{
    cache.retain(|_, value| now.saturating_duration_since(inserted_at(value)) <= ttl);
    if cache.len() <= max_entries {
        return;
    }
    let excess = cache.len() - max_entries;
    let mut entries: Vec<(String, Instant)> = cache
        .iter()
        .map(|(k, v)| (k.clone(), inserted_at(v)))
        .collect();
    entries.sort_unstable_by_key(|(_, ts)| *ts);
    for (oldest_key, _) in entries.into_iter().take(excess) {
        cache.remove(&oldest_key);
    }
}
