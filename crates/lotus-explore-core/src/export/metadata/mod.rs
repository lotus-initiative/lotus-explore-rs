// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Schema.org / BioSchemas metadata document construction for exports.

mod builder;
mod schema;

pub use builder::build_metadata_json;
use lotus_model::{SearchCriteria, SparqlEndpoint};

/// The version the metadata document reports.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
/// The name the metadata document reports.
pub const APP_NAME: &str = "LOTUS Explorer";
/// From the manifest, so the URL is declared once for the whole workspace.
const APP_URL: &str = env!("CARGO_PKG_REPOSITORY");

/// Everything the metadata document is built from.
#[derive(Debug)]
pub struct MetadataInputs<'a> {
    /// The search the document describes.
    pub criteria: &'a SearchCriteria,
    /// The QID the taxon resolved to, if it resolved to one.
    pub qid: Option<&'a str>,
    /// How many records the set holds, when the set was fetched.
    pub number_of_records_override: Option<usize>,
    /// The hash of the query that ran.
    pub query_hash: &'a str,
    /// The hash of the rows that came back.
    pub result_hash: &'a str,
    /// The SPARQL endpoint used for this query (Qlever by default, WDQS on fallback)
    pub endpoint: SparqlEndpoint,
    /// The SPARQL query text that was executed, when available.
    pub sparql_query: Option<&'a str>,
    /// The current year, which decides whether the year bounds mean anything.
    pub year: u16,
    /// The instant the export was produced, as an ISO-8601 stamp.
    pub date_created: &'a str,
}

#[cfg(test)]
use serde_json::{Value, json};

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
