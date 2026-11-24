// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Result finalization service.

use crate::explore::hashing::compute_hashes;
use crate::export::metadata::MetadataInputs;
use lotus_model::{ColumnarResultSet, SearchCriteria, SparqlEndpoint};
use std::sync::Arc;

/// Computed hashes and metadata JSON for a single search result.
#[derive(Clone, Debug)]
pub struct FinalizedMeta {
    /// Hash of the query that produced the result.
    pub query_hash: Arc<str>,
    /// Hash of the result set.
    pub result_hash: Arc<str>,
    /// Export metadata JSON for the result.
    pub metadata_json: Arc<str>,
}

/// Inputs needed to compute query/result hashes and export metadata.
#[derive(Clone, Debug)]
pub struct FinalizeInputs<'a> {
    /// Search criteria used for the query.
    pub criteria: &'a SearchCriteria,
    /// Resolved taxon QID, if any.
    pub qid: Option<&'a str>,
    /// Result rows produced by the search.
    pub set: &'a ColumnarResultSet,
    /// Whether the run was triggered as direct download only.
    pub direct_download_mode: bool,
    /// SPARQL endpoint that served the query.
    pub endpoint: SparqlEndpoint,
    /// Raw SPARQL query text, if captured.
    pub sparql_query: Option<&'a str>,
    /// Current calendar year used for default year-bound comparisons.
    pub year: u16,
    /// ISO-8601 timestamp recorded in export metadata.
    pub now: &'a str,
}

/// Assemble [`FinalizedMeta`] from the raw outcome parts.
///
/// `direct_download_mode` suppresses stats/counts, which were never fetched. The
/// `set` carries its own exact counts, so nothing is recomputed: the numbers the
/// reader sees are the numbers the fetch produced.
#[must_use]
#[allow(
    clippy::needless_pass_by_value,
    reason = "callers pass inline FinalizeInputs literals by value"
)]
pub fn finalize(inputs: FinalizeInputs<'_>) -> FinalizedMeta {
    // Absent in download-only mode: no result was fetched, and the empty set's zero
    // would claim the search matched nothing rather than that it never ran.
    let number_of_records = (!inputs.direct_download_mode).then_some(inputs.set.stats().n_entries);

    let (query_hash, result_hash) = compute_hashes(
        inputs.qid.unwrap_or(""),
        inputs.criteria,
        inputs.set,
        inputs.year,
    );
    let metadata_json = Arc::<str>::from(crate::export::metadata::build_metadata_json(
        MetadataInputs {
            criteria: inputs.criteria,
            qid: inputs.qid,
            number_of_records_override: number_of_records,
            query_hash: &query_hash,
            result_hash: &result_hash,
            endpoint: inputs.endpoint,
            sparql_query: inputs.sparql_query,
            year: inputs.year,
            date_created: inputs.now,
        },
    ));

    FinalizedMeta {
        query_hash: Arc::from(query_hash),
        result_hash: Arc::from(result_hash),
        metadata_json,
    }
}

#[cfg(test)]
#[path = "finalize/tests.rs"]
mod tests;
