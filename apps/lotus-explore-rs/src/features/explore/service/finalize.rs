// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Result finalization service.

use crate::export;
use crate::features::explore::search_utils::compute_hashes;
use lotus_model::{CompoundEntry, DatasetStats, SearchCriteria};
use std::sync::Arc;

/// Computed hashes and metadata JSON for a single search result.
pub struct FinalizedMeta {
    pub query_hash: Arc<str>,
    pub result_hash: Arc<str>,
    pub metadata_json: Arc<str>,
    /// Filtered match count (absent in download-only mode).
    pub filtered_matches: Option<usize>,
    /// Filtered dataset stats (absent in download-only mode).
    pub filtered_stats: Option<DatasetStats>,
}

/// Assemble [`FinalizedMeta`] from the raw outcome parts.
/// `direct_download_mode` suppresses stats/counts (they were never fetched).
pub fn finalize(
    crit: &SearchCriteria,
    qid: Option<&str>,
    rows: &[CompoundEntry],
    raw_matches: Option<usize>,
    raw_stats: Option<DatasetStats>,
    direct_download_mode: bool,
    endpoint: export::SparqlEndpoint,
) -> FinalizedMeta {
    let filtered_stats = if direct_download_mode {
        None
    } else {
        Some(raw_stats.unwrap_or_else(|| DatasetStats::from_entries(rows)))
    };
    let filtered_matches = if direct_download_mode {
        None
    } else {
        Some(raw_matches.unwrap_or(rows.len()))
    };

    let (query_hash, result_hash) = compute_hashes(qid.unwrap_or(""), crit, rows);
    let metadata_json = Arc::<str>::from(export::build_metadata_json(export::MetadataInputs {
        criteria: crit,
        qid,
        number_of_records_override: filtered_matches,
        query_hash: &query_hash,
        result_hash: &result_hash,
        endpoint,
    }));

    FinalizedMeta {
        query_hash: Arc::from(query_hash),
        result_hash: Arc::from(result_hash),
        metadata_json,
        filtered_matches,
        filtered_stats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lotus_search::SearchCriteria;

    #[test]
    fn download_only_suppresses_stats_and_matches() {
        let crit = SearchCriteria::up_to_year(crate::clock::current_year());
        let m = finalize(
            &crit,
            None,
            &[],
            None,
            None,
            true,
            export::SparqlEndpoint::Qlever,
        );
        assert!(m.filtered_matches.is_none());
        assert!(m.filtered_stats.is_none());
    }

    #[test]
    fn normal_mode_fills_stats_and_matches() {
        let crit = SearchCriteria::up_to_year(crate::clock::current_year());
        let m = finalize(
            &crit,
            None,
            &[],
            Some(7),
            None,
            false,
            export::SparqlEndpoint::Qlever,
        );
        assert_eq!(m.filtered_matches, Some(7));
        assert!(m.filtered_stats.is_some());
    }

    #[test]
    fn hashes_are_deterministic() {
        let crit = SearchCriteria::up_to_year(crate::clock::current_year());
        let m1 = finalize(
            &crit,
            Some("Q42"),
            &[],
            None,
            None,
            false,
            export::SparqlEndpoint::Qlever,
        );
        let m2 = finalize(
            &crit,
            Some("Q42"),
            &[],
            None,
            None,
            false,
            export::SparqlEndpoint::Qlever,
        );
        assert_eq!(m1.query_hash, m2.query_hash);
        assert_eq!(m1.result_hash, m2.result_hash);
    }

    #[test]
    fn metadata_json_is_non_empty() {
        let crit = SearchCriteria::up_to_year(crate::clock::current_year());
        let m = finalize(
            &crit,
            None,
            &[],
            None,
            None,
            false,
            export::SparqlEndpoint::Qlever,
        );
        // `assert!` rather than `assert_eq!(len, 0)`: clippy 1.99 added
        // `assert_is_empty` to pedantic, which this workspace denies, and this is
        // the shape it flags. It also reads as the claim the test makes.
        assert!(!m.metadata_json.is_empty(), "the metadata JSON is written");
    }
}
