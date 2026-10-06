// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Result finalization service.

use crate::export;
use crate::features::explore::search_utils::compute_hashes;
use lotus_model::{ColumnarResultSet, SearchCriteria};
use std::sync::Arc;

/// Computed hashes and metadata JSON for a single search result.
pub struct FinalizedMeta {
    pub query_hash: Arc<str>,
    pub result_hash: Arc<str>,
    pub metadata_json: Arc<str>,
}

/// Assemble [`FinalizedMeta`] from the raw outcome parts.
/// `direct_download_mode` suppresses stats/counts, which were never fetched. The
/// `set` carries its own exact counts, so nothing is recomputed: the numbers the
/// reader sees are the numbers the fetch produced.
pub fn finalize(
    crit: &SearchCriteria,
    qid: Option<&str>,
    set: &ColumnarResultSet,
    direct_download_mode: bool,
    endpoint: export::SparqlEndpoint,
) -> FinalizedMeta {
    // Absent in download-only mode: no result was fetched, and the empty set's zero
    // would claim the search matched nothing rather than that it never ran.
    let number_of_records = (!direct_download_mode).then_some(set.stats().n_entries);

    let (query_hash, result_hash) = compute_hashes(qid.unwrap_or(""), crit, set);
    let metadata_json = Arc::<str>::from(export::build_metadata_json(export::MetadataInputs {
        criteria: crit,
        qid,
        number_of_records_override: number_of_records,
        query_hash: &query_hash,
        result_hash: &result_hash,
        endpoint,
    }));

    FinalizedMeta {
        query_hash: Arc::from(query_hash),
        result_hash: Arc::from(result_hash),
        metadata_json,
    }
}

#[cfg(test)]
#[path = "finalize/tests.rs"]
mod tests;
