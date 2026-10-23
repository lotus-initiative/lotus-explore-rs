// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Pure URL codec for explore state.

use lotus_search::SearchCriteria;
use std::collections::BTreeMap;

mod criteria;
mod encode;
mod startup;

pub use criteria::{criteria_query_params, parse_criteria_from_params};
pub use encode::build_shareable_url;
pub use startup::{InitialDownloadState, parse_startup_action_from_params};

/// The URL query parameters of a search, keyed by parameter name.
pub type QueryParams = BTreeMap<String, String>;

/// The startup state decoded from a URL, before the adapter maps the locale.
#[derive(Clone, Debug, PartialEq)]
pub struct InitialUrlState {
    /// The search criteria decoded from the URL.
    pub criteria: SearchCriteria,
    /// The raw locale selector, if the URL carried one.
    pub locale: u8,
    /// The download or execute action requested at startup.
    pub download: InitialDownloadState,
    /// Whether dark mode was requested.
    pub dark_mode: bool,
}

/// Test whether a URL query-parameter value represents a boolean true flag.
///
/// Accepts `"1"`, `"true"`, `"yes"`, and `"on"` (case-insensitive, trimmed).
/// All other values — including absent keys — are treated as false.
#[must_use]
pub fn is_true_flag(v: &str) -> bool {
    let t = v.trim();
    t == "1"
        || t.eq_ignore_ascii_case("true")
        || t.eq_ignore_ascii_case("yes")
        || t.eq_ignore_ascii_case("on")
}

#[cfg(test)]
mod tests;
