// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Explore feature domain.

pub use form_actions::{FormAction, apply_form_action};
pub use search_state::ExploreState;
pub use types::{DomainError, ErrorKind, LookupNotice, ParseFault, QueryStage, ValidationFault};

pub mod actions;
pub mod command;
pub mod error_recovery;
pub mod form_actions;
pub mod form_validation;
pub mod outcome;
pub mod recovery;
pub mod request;
pub mod retry;
pub mod search_metrics;
pub mod search_state;
/// Shared string and hash helpers for search inputs and results.
pub mod search_utils;
pub mod service;
pub mod sparql_errors;
/// Classification of transport failures shared by retry, UI hints, and telemetry.
pub mod transport_classification;
pub mod truncation;
pub mod types;
pub mod url_codec;
