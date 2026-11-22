// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Bounded query-criteria history for graph pivots (`Explore in LOTUS`).
//!
//! Stores only lightweight [`lotus_model::SearchCriteria`] entries — never
//! [`lotus_model::ColumnarResultSet`] — so browsing dozens of connected subgraphs
//! never retains megabytes of columnar buffers in memory.

use std::collections::VecDeque;

/// Maximum number of prior searches kept for the `Previous search` action.
pub const MAX_PIVOT_HISTORY: usize = 20;

/// Bounded LIFO stack of previous [`lotus_model::SearchCriteria`] values.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct PivotHistory {
    entries: VecDeque<lotus_model::SearchCriteria>,
}

impl PivotHistory {
    /// Create an empty history stack.
    pub const fn new() -> Self {
        Self {
            entries: VecDeque::new(),
        }
    }

    /// Record `criteria` before starting a new search, evicting the oldest entry
    /// if the stack has reached [`MAX_PIVOT_HISTORY`].
    pub fn push(&mut self, criteria: lotus_model::SearchCriteria) {
        if self.entries.len() >= MAX_PIVOT_HISTORY {
            let _ = self.entries.pop_front();
        }
        self.entries.push_back(criteria);
    }

    /// Pop the most recent previous search criteria, if any.
    pub fn pop(&mut self) -> Option<lotus_model::SearchCriteria> {
        self.entries.pop_back()
    }

    /// Number of stored history entries.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no prior search criteria are stored.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
