// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Result finalization service.

use crate::explore::search_utils::compute_hashes;
use crate::export::metadata::{MetadataInputs, SparqlEndpoint};
use lotus_model::{ColumnarResultSet, SearchCriteria};
use std::sync::Arc;

/// Computed hashes and metadata JSON for a single search result.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct FinalizedMeta {
    /// Hash of the query that produced the result.
    pub query_hash: Arc<str>,
    /// Hash of the result set.
    pub result_hash: Arc<str>,
    /// Export metadata JSON for the result.
    pub metadata_json: Arc<str>,
}

/// Assemble [`FinalizedMeta`] from the raw outcome parts.
///
/// `direct_download_mode` suppresses stats/counts, which were never fetched. The
/// `set` carries its own exact counts, so nothing is recomputed: the numbers the
/// reader sees are the numbers the fetch produced.
#[must_use]
pub fn finalize(
    crit: &SearchCriteria,
    qid: Option<&str>,
    set: &ColumnarResultSet,
    direct_download_mode: bool,
    endpoint: SparqlEndpoint,
    year: u16,
    now: &str,
) -> FinalizedMeta {
    // Absent in download-only mode: no result was fetched, and the empty set's zero
    // would claim the search matched nothing rather than that it never ran.
    let number_of_records = (!direct_download_mode).then_some(set.stats().n_entries);

    let (query_hash, result_hash) = compute_hashes(qid.unwrap_or(""), crit, set, year);
    let metadata_json = Arc::<str>::from(crate::export::metadata::build_metadata_json(
        MetadataInputs {
            criteria: crit,
            qid,
            number_of_records_override: number_of_records,
            query_hash: &query_hash,
            result_hash: &result_hash,
            endpoint,
            year,
            date_created: now,
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
