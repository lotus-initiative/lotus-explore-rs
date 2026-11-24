// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Turning a [`SearchCriteria`](lotus_model::SearchCriteria) into a search result.
//!
//! This is the sequence the web app, the CLI and the API server all need:
//! resolve the taxon, build the query, apply the filters, execute, parse, count.
//!
//! Each step is a separate function so a caller can stop early — the web app
//! needs the query on its own for the "download the query" button, and the
//! server needs the counts without the rows.

mod execute_search;
mod query_plan;
mod resolve_reference;
mod resolve_structure;
mod resolve_taxon;
mod stream_search;

#[cfg(test)]
mod tests;

pub use execute_search::{DEFAULT_ROW_LIMIT, SearchError, counts, search};
pub use query_plan::{
    ResolvedInputs, StructurePlan, build_base_query, build_execution_query,
    build_execution_query_with,
};
pub use resolve_reference::resolve_reference;
pub use resolve_structure::{StructureResolution, resolve_inputs, resolve_structure};
pub use resolve_taxon::resolve_taxon;
pub use stream_search::{
    StreamProgress, columnar_from_chunks, columnar_from_chunks_reporting, enrich_columnar_reader,
    feed_chunks_reporting, search_columnar,
};

#[cfg(test)]
use resolve_taxon::is_ambiguous;
