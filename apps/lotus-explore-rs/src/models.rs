// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! The app's view of the domain.
//!
//! Everything in `lotus-model` is re-exported so that the app has one word for
//! the vocabulary, and the few things that are the app's own rather than the
//! domain's are named here: the clock, the row budget, and sort state. A reader
//! arriving at `crate::models::` should be able to tell which is which.

pub use lotus_model::*;

/// The oldest publication year the app will offer as a lower bound.
///
/// Nothing in natural-products literature is older, so a reference dated before
/// this is a parse error rather than a very old paper, and the year input uses
/// it as its floor.
pub const DEFAULT_YEAR_MIN: u16 = 1_800;

// The per-element ceilings come from `lotus_model::element_max` rather than
// being restated here, so that the number shown as a form maximum and the
// number a filter is validated against cannot drift apart.
pub use lotus_model::element_max::{
    C as DEFAULT_C_MAX, H as DEFAULT_H_MAX, N as DEFAULT_N_MAX, O as DEFAULT_O_MAX,
    P as DEFAULT_P_MAX, S as DEFAULT_S_MAX,
};

pub use crate::clock::current_year;
pub use crate::sort::{SortColumn, SortDir, SortState};
pub use crate::table_budget::runtime_table_row_limit;
