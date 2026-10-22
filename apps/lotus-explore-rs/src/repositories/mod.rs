// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Repository layer: a thin boundary between search orchestration and transport.
//!
//! The port itself — the trait the domain asks, and the error it is told about
//! — lives in `lotus-explore-core`, because the domain is what defines it.
//! What is here is the implementation: the hybrid repository that talks to the
//! REST API and to SPARQL, and the mock the tests use.

pub mod hybrid;
#[cfg(test)]
pub mod mock;

// Re-export WDQS fallback tracking functions from hybrid.rs
pub use hybrid::{get_wdqs_transformed_query, is_wdqs_fallback_used, reset_wdqs_fallback_flag};

pub use hybrid::HybridRepository;

pub use lotus_explore_core::ports::{LotusRepository, RepositoryError};
