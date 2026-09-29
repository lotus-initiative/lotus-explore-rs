// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Per-column cell render functions for results-table rows.

mod compound;
mod numeric;
mod reference;
mod structure;
mod taxon;

pub(in crate::components::results_table::row_cells) use compound::compound_cell;
pub(in crate::components::results_table::row_cells) use numeric::{
    formula_cell, mass_cell, year_cell,
};
pub(in crate::components::results_table::row_cells) use reference::reference_cell;
pub(in crate::components::results_table::row_cells) use structure::structure_cell;
pub(in crate::components::results_table::row_cells) use taxon::taxon_cell;
