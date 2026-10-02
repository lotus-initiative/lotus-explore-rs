// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Fixtures and structural helpers shared by the query-contract tests.
//!
//! The tests assert structure rather than bytes: which subquery a fragment lands
//! in, which `OPTIONAL`s the count query drops, which triple the filter injection
//! makes required. Layout may change; the query plan may not. That is why these
//! helpers count `SELECT`s and strip a trailing brace instead of comparing
//! against golden strings.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "a test that fails on a bad fixture is reporting, not panicking"
)]

use lotus_model::SearchCriteria;
use lotus_query::{compounds_by_taxon_query, with_filters};

/// The reference year the fixtures are built around.
pub const NOW: u16 = 2026;

/// The baseline criteria: every filter unset.
#[must_use]
pub const fn criteria() -> SearchCriteria {
    SearchCriteria::up_to_year(NOW)
}

/// `SELECT` opens a subquery at each of these depths.
#[must_use]
pub fn subquery_depth(query: &str) -> usize {
    query.matches("SELECT").count()
}

/// What the base query looks like with its own closing brace removed, which is
/// what the filter fragments are spliced onto.
///
/// # Panics
/// If the query does not end with `}`. A built query always does, so this is
/// asserting the shape of the builder's output rather than handling input.
#[must_use]
pub fn base_body(query: &str) -> &str {
    query
        .trim_end()
        .strip_suffix('}')
        .expect("a built query ends with `}`")
}

/// The formula filter emits a `FILTER(?_count_c >= … && <= …)` only when the
/// criterion actually constrains carbon. When it does not, the filter is noise
/// that still costs a cache key, because a query's bytes are what the cache key
/// and a shared link are derived from.
pub(crate) fn carbon_ranged_filter(c_min: u16, c_max: u16) -> Option<String> {
    let query = with_filters(
        &compounds_by_taxon_query("Q16521"),
        &SearchCriteria {
            formula_enabled: true,
            // Something has to make the formula filter run at all, or the element
            // loop is never reached and the guard cannot be distinguished from
            // any other spelling of it. An exact formula does that without
            // touching the carbon bounds under test.
            formula_exact: "CCO".into(),
            c_min,
            c_max,
            ..criteria()
        },
        NOW,
    );
    query
        .lines()
        .find(|line| line.contains("?_count_c >="))
        .map(str::trim)
        .map(str::to_string)
}
