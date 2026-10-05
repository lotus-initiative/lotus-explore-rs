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
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use lotus_model::{ColumnarResultSet, CompoundEntry};
    use lotus_search::SearchCriteria;
    use std::sync::Arc;

    /// The value of `key` in the metadata JSON, or `None` if it is absent.
    ///
    /// The document is pretty-printed, so a substring test for `"k": v` would have to
    /// guess about whitespace.
    fn record_count(meta: &FinalizedMeta) -> Option<usize> {
        let json: serde_json::Value =
            serde_json::from_str(&meta.metadata_json).expect("metadata is JSON");
        json.get("numberOfRecords")
            .and_then(serde_json::Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
    }

    fn set_of(n: usize) -> Arc<ColumnarResultSet> {
        let rows: Vec<CompoundEntry> = (0..n)
            .map(|i| CompoundEntry {
                compound_qid: Arc::from(format!("Q{i}")),
                name: Arc::from(format!("Compound {i}")),
                ..Default::default()
            })
            .collect();
        Arc::new(ColumnarResultSet::from_entries(&rows))
    }

    #[test]
    fn download_only_suppresses_stats_and_matches() {
        let crit = SearchCriteria::up_to_year(crate::clock::current_year());
        let m = finalize(
            &crit,
            None,
            &set_of(3),
            true,
            export::SparqlEndpoint::Qlever,
        );
        assert_eq!(
            record_count(&m),
            None,
            "download-only reports no record count at all: no result was fetched, \
             and a zero would claim the search matched nothing"
        );
    }

    #[test]
    fn the_counts_come_from_the_set_and_never_disagree_with_it() {
        // There is no count argument, which is the point: a field that could say 7 while
        // the set held 3 is the bug the endpoint's `COUNT` query used to produce.
        let crit = SearchCriteria::up_to_year(crate::clock::current_year());
        let set = set_of(3);
        let m = finalize(&crit, None, &set, false, export::SparqlEndpoint::Qlever);

        assert_eq!(
            record_count(&m),
            Some(3),
            "the record count is the set's own count"
        );
    }

    #[test]
    fn an_empty_search_reports_zero_rather_than_no_answer() {
        let crit = SearchCriteria::up_to_year(crate::clock::current_year());
        let m = finalize(
            &crit,
            None,
            &set_of(0),
            false,
            export::SparqlEndpoint::Qlever,
        );

        assert_eq!(
            record_count(&m),
            Some(0),
            "an empty search reports zero, not an absent count"
        );
    }
}
