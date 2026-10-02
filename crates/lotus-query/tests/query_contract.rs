// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The SPARQL each builder must produce, split by concern.
//!
//! Four files, because a 1,000-line test binary makes "where does this test go" a
//! question you answer by reading all of it:
//!
//! - [`contract::nomenclature`] the four relationships a taxon search follows.
//! - [`contract::structure`] structure search, similarity and substructure.
//! - [`contract::filters`] filter injection and the count query.
//! - [`contract::export`] the shape of the base queries and their variants.
//!
//! They assert structure rather than bytes: which subquery a fragment lands in,
//! which `OPTIONAL`s the count query drops, which triple the filter injection makes
//! required. Layout may change; the query plan may not.

#![allow(
    unused_crate_dependencies,
    reason = "the test binary links the crate's deps without using them all"
)]

pub mod contract {
    //! The query-contract tests, split by concern.

    pub mod common;
    pub mod export;
    pub mod filters;
    pub mod nomenclature;
    pub mod structure;
}
