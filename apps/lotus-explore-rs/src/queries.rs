// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Building a query, and reading the answer.
//!
//! `lotus-query` holds both halves, because a query and the CSV it comes back
//! as are two ends of one conversation with the endpoint. This file exists to
//! give the app one place to reach for either.

pub use lotus_model::{SearchCriteria, StructureKind, classify_structure};
pub use lotus_query::*;

// The names the app grew up with, kept so that a call site can say what it
// wants rather than which function provides it. Each one is the same operation
// under the name the surrounding code already uses.
pub use lotus_query::all_compounds_query as query_all_compounds;
pub use lotus_query::compounds_by_taxon_query as query_compounds_by_taxon;
pub use lotus_query::structure_search_query as query_sachem;
pub use lotus_query::taxon_lookup_query as query_taxon_search;

/// Apply the criteria's filters to a base query.
///
/// `lotus-query` takes the current year as an argument so that it stays pure and
/// its tests can pin a year. The app has a clock, so it is the one that supplies
/// it, here rather than at each of the call sites.
#[must_use]
pub fn query_with_server_filters(base_query: &str, criteria: &SearchCriteria) -> String {
    lotus_query::with_filters(base_query, criteria, crate::models::current_year())
}
