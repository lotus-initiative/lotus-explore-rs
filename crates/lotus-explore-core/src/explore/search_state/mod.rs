// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Explore feature state, reducer, and dispatch helpers.
//! Split into focused submodules to keep this facade short and maintainable.

mod reducer;
mod state_types;

#[cfg(test)]
pub use reducer::reduce;
pub use reducer::reduce_mut;
pub use state_types::{ExploreState, ResultDataState, SearchLifecycleState, UiChromeState};

#[cfg(test)]
mod tests;
