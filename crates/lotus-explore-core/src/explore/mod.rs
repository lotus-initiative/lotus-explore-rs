// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Explore feature domain.

pub mod actions;
pub mod command;
pub mod error;
mod error_recovery;
pub mod form_actions;
pub mod form_validation;
/// Query and result-set SHA-256 hashing for shareable URLs and export provenance.
mod hashing;
pub mod outcome;
pub mod recovery;
pub mod request;
pub mod retry;
pub mod search_metrics;
pub mod search_state;
pub mod service;
mod sparql_errors;
/// Classification of transport failures shared by retry, UI hints, and telemetry.
pub mod transport_classification;
pub mod truncation;
pub mod url_codec;
