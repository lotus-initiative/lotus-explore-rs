// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

// The curation vocabulary lives in `lotus-curation`, shared with the CLI.
pub mod queue;
pub mod repositories;
pub mod services;
pub mod state;
pub mod workflow;

pub use state::page_controller::use_curation_page_controller;
