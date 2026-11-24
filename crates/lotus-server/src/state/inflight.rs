// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![expect(
    clippy::redundant_pub_crate,
    reason = "items are re-exported pub(crate) to sibling modules from a private submodule"
)]
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

pub(crate) fn search_inflight_cell(
    state: &AppState,
    key: &str,
) -> Result<(InFlightSearch, bool), ApiError> {
    acquire_inflight_cell(
        &state.search_inflight,
        key,
        "search inflight mutex poisoned",
    )
}

pub(crate) fn search_inflight_remove(state: &AppState, key: &str, cell: &InFlightSearch) {
    remove_inflight_cell(&state.search_inflight, key, cell);
}

pub(crate) fn export_inflight_cell(
    state: &AppState,
    key: &str,
) -> Result<(InFlightExport, bool), ApiError> {
    acquire_inflight_cell(
        &state.export_inflight,
        key,
        "export inflight mutex poisoned",
    )
}

pub(crate) fn export_inflight_remove(state: &AppState, key: &str, cell: &InFlightExport) {
    remove_inflight_cell(&state.export_inflight, key, cell);
}
