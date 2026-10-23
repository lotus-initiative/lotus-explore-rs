// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Explore feature facade.

pub use download_dispatch::{use_download_dispatch_effect, use_startup_effect};
pub use interactions::{ExploreInteractions, use_explore_interactions};
pub use orchestrator::SearchTaskController;
pub use selectors::{
    ExploreUiState, use_criteria_selector, use_header_meta_snapshot, use_lifecycle_selector,
    use_toolbar_result_snapshot,
};
pub use url_state::{
    InitialUrlState, absolute_current_url_with_query, absolute_share_url, build_shareable_url,
    initial_url_state, is_true_flag,
};

#[cfg(test)]
pub use url_state::InitialDownloadState;

pub mod download_dispatch;
pub mod download_effects;
pub mod executor;
pub mod interactions;
pub mod lifecycle;
pub mod orchestrator;
pub mod search_state;
pub mod selectors;
pub mod service;
pub mod structure_cache;
pub mod taxon_cache;
pub mod url_state;
