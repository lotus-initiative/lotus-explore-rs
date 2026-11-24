// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Curation against a live Wikidata.
//!
//! The queries are in `wikidata_query`; this runs them. The transport is the
//! [`Http`](lotus_search::Http) trait from `lotus-search`, so curation is tested
//! against a scripted conversation.
//!
//! Nothing here submits. It reads what is there, reports the difference, and the
//! statements it produces are for a person to review.

mod dependencies;
mod lookup;
mod result_row;
mod statements;

#[cfg(test)]
use crate::{CurationError, CurationInputRow, CurationStatus, WikidataCompound};
#[cfg(test)]
use dependencies::taxon_dependency_statements;
#[cfg(test)]
use lookup::{ask, binding_qid, first_bindings};

#[cfg(test)]
mod tests;

pub use lookup::{StructureKey, WikidataLookup, look_up};
pub use result_row::to_result_row;
pub use statements::{REVISED_ROW_NOTE, is_revised_row, revised_row_statement};
