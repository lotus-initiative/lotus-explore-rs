// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Subcomponents for the `ResultsTable` toolbar sections.
//! Each submodule owns exactly one visual concern:
//! More detail in the type and function docs below.

mod download_actions;
mod query_panel;
mod stat_bar;

pub use download_actions::DownloadActionsGroup;
pub use query_panel::QueryPanel;
pub use stat_bar::{CappedRowsNotice, StatBar};
