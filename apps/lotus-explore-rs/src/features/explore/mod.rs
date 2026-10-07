// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Explore feature facade.

pub use download_dispatch::{use_download_dispatch_effect, use_startup_effect};
pub use form_actions::FormAction;
#[cfg(test)]
pub use form_actions::apply_form_action;
pub use interactions::{ExploreInteractions, use_explore_interactions};
pub use orchestrator::SearchTaskController;
pub use search_state::ExploreState;
pub use selectors::{
    ExploreUiState, use_criteria_selector, use_header_meta_snapshot, use_lifecycle_selector,
    use_toolbar_result_snapshot,
};
pub use types::{DomainError, ErrorKind, LookupNotice, ParseFault, QueryStage, ValidationFault};
pub use url_state::{
    InitialUrlState, absolute_current_url_with_query, absolute_share_url, build_shareable_url,
    initial_url_state, is_true_flag,
};

#[cfg(test)]
pub use url_state::InitialDownloadState;

pub mod actions;
pub mod command;
pub mod download_dispatch;
pub mod download_effects;
pub mod error_recovery_coordinator;
pub mod executor;
pub mod form_actions;
pub mod form_validation;
pub mod interactions;
pub mod lifecycle;
pub mod orchestrator;
pub mod outcome;
pub mod recovery;
pub mod request;
pub mod retryable_orchestrator;
pub mod search_metrics;
pub mod search_state;
pub mod search_utils;
pub mod selectors;
pub mod service;
mod sparql_errors;
pub mod structure_cache;
pub mod taxon_cache;
mod transport_classification;
pub mod truncation;
pub mod types;
mod url_codec;
pub mod url_state;
