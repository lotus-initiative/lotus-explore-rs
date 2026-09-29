// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Pure URL codec for explore state.

use crate::i18n::Locale;
use crate::models::SearchCriteria;
use std::collections::BTreeMap;

mod criteria;
mod encode;
mod startup;

pub use criteria::parse_criteria_from_params;
pub use encode::build_shareable_url;
pub use startup::{InitialDownloadState, parse_startup_action_from_params};

pub type QueryParams = BTreeMap<String, String>;

#[derive(Clone, Debug, PartialEq)]
pub struct InitialUrlState {
    pub criteria: SearchCriteria,
    pub locale: Locale,
    pub download: InitialDownloadState,
    pub dark_mode: bool,
}

/// Test whether a URL query-parameter value represents a boolean true flag.
/// Accepts `"1"`, `"true"`, `"yes"`, and `"on"` (case-insensitive, trimmed).
/// All other values — including absent keys — are treated as false.
pub fn is_true_flag(v: &str) -> bool {
    let t = v.trim();
    t == "1"
        || t.eq_ignore_ascii_case("true")
        || t.eq_ignore_ascii_case("yes")
        || t.eq_ignore_ascii_case("on")
}

#[cfg(test)]
mod tests;
