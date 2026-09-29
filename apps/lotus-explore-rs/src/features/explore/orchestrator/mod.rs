// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Search orchestration — thin dispatcher that sequences service calls.

mod controller;
mod runtime;

pub use controller::SearchTaskController;
pub use runtime::start_search;

#[cfg(test)]
mod tests;
