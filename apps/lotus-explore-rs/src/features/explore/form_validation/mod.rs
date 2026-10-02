// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Form input validation framework.
//! Centralizes validation logic for search form inputs to enable:
//! More detail in the type and function docs below.

mod dispatch;
mod rules;
mod types;

pub use dispatch::{is_unconstrained, validate_dispatch_criteria};

#[cfg(test)]
mod tests;
