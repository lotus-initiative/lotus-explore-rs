// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Single-flight `OnceCell` coalescing helpers for concurrent identical requests.

use std::{
    collections::{HashMap, hash_map::Entry},
    sync::{Arc, Mutex},
};

use tokio::sync::OnceCell;

use super::{AppState, InFlightExport, InFlightSearch};
use crate::errors::ApiError;

fn acquire_inflight_cell<T>(
    map: &Mutex<HashMap<String, Arc<OnceCell<T>>>>,
    key: &str,
    poison_message: &'static str,
) -> Result<(Arc<OnceCell<T>>, bool), ApiError> {
    let mut guard = map.lock().map_err(|_| ApiError::internal(poison_message))?;
    Ok(match guard.entry(key.to_string()) {
        Entry::Occupied(entry) => (entry.get().clone(), false),
        Entry::Vacant(entry) => {
            let cell = Arc::new(OnceCell::new());
            entry.insert(cell.clone());
            (cell, true)
        }
    })
}

fn remove_inflight_cell<T>(
    map: &Mutex<HashMap<String, Arc<OnceCell<T>>>>,
    key: &str,
    cell: &Arc<OnceCell<T>>,
) {
    if let Ok(mut inflight) = map.lock()
        && inflight
            .get(key)
            .is_some_and(|current| Arc::ptr_eq(current, cell))
    {
        inflight.remove(key);
    }
}

/// Drops an in-flight cell from `map` if all callers finished or dropped early.
pub struct InflightGuard<T> {
    map: Arc<Mutex<HashMap<String, Arc<OnceCell<T>>>>>,
    key: String,
    cell: Arc<OnceCell<T>>,
    is_leader: bool,
    completed: bool,
}

impl<T> InflightGuard<T> {
    #[must_use]
    pub const fn cell(&self) -> &Arc<OnceCell<T>> {
        &self.cell
    }

    #[must_use]
    pub const fn is_leader(&self) -> bool {
        self.is_leader
    }

    /// Explicitly remove the entry once `cell.get_or_init(...)` finishes cleanly.
    pub fn finish(mut self) {
        self.completed = true;
        remove_inflight_cell(&self.map, &self.key, &self.cell);
    }
}

impl<T> Drop for InflightGuard<T> {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        // Only remove on early cancellation when this guard holds the last active
        // external reference besides the map entry itself (strong_count <= 2).
        if let Ok(mut inflight) = self.map.lock()
            && inflight.get(&self.key).is_some_and(|current| {
                Arc::ptr_eq(current, &self.cell) && Arc::strong_count(current) <= 2
            })
        {
            inflight.remove(&self.key);
        }
    }
}

pub fn search_inflight_guard(
    state: &AppState,
    key: &str,
) -> Result<
    InflightGuard<Result<crate::dto::SearchResponse, crate::errors::SharedApiError>>,
    ApiError,
> {
    let (cell, is_leader) = search_inflight_cell(state, key)?;
    Ok(InflightGuard {
        map: Arc::clone(&state.search_inflight),
        key: key.to_string(),
        cell,
        is_leader,
        completed: false,
    })
}

pub fn export_inflight_guard(
    state: &AppState,
    key: &str,
) -> Result<
    InflightGuard<Result<crate::dto::ExportUrlResponse, crate::errors::SharedApiError>>,
    ApiError,
> {
    let (cell, is_leader) = export_inflight_cell(state, key)?;
    Ok(InflightGuard {
        map: Arc::clone(&state.export_inflight),
        key: key.to_string(),
        cell,
        is_leader,
        completed: false,
    })
}

pub fn search_inflight_cell(
    state: &AppState,
    key: &str,
) -> Result<(InFlightSearch, bool), ApiError> {
    acquire_inflight_cell(
        &state.search_inflight,
        key,
        "search inflight mutex poisoned",
    )
}

#[cfg(test)]
pub fn search_inflight_remove(state: &AppState, key: &str, cell: &InFlightSearch) {
    remove_inflight_cell(&state.search_inflight, key, cell);
}

pub fn export_inflight_cell(
    state: &AppState,
    key: &str,
) -> Result<(InFlightExport, bool), ApiError> {
    acquire_inflight_cell(
        &state.export_inflight,
        key,
        "export inflight mutex poisoned",
    )
}

#[cfg(test)]
pub fn export_inflight_remove(state: &AppState, key: &str, cell: &InFlightExport) {
    remove_inflight_cell(&state.export_inflight, key, cell);
}
