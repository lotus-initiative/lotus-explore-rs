// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! UI components and contracts for the application.

pub mod a11y_contract;
mod a11y_smoke;
pub mod common;
pub mod notice;
pub mod segmented_control;

pub use common::{ContentPhase, LifecycleBooleans};

pub mod prelude {
    pub use super::notice::*;
    pub use super::segmented_control::*;
}
