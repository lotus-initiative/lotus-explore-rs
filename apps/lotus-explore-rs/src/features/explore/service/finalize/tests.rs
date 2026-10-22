// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `finalize`, in their own file.

#![allow(clippy::expect_used)]

use super::*;
use lotus_model::{ColumnarResultSet, CompoundEntry};
use lotus_search::SearchCriteria;
use std::sync::Arc;

/// The year the tests pin, so "is this year bound in the future?" has one
/// answer rather than the answer the day the suite runs.
const YEAR: u16 = 2026;

/// The instant the tests pin, so the document they assert is the whole one.
const NOW: &str = "2026-03-04T05:06:07Z";

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
    let crit = SearchCriteria::up_to_year(YEAR);
    let m = finalize(
        &crit,
        None,
        &set_of(3),
        true,
        SparqlEndpoint::Qlever,
        YEAR,
        NOW,
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
    let crit = SearchCriteria::up_to_year(YEAR);
    let set = set_of(3);
    let m = finalize(&crit, None, &set, false, SparqlEndpoint::Qlever, YEAR, NOW);

    assert_eq!(
        record_count(&m),
        Some(3),
        "the record count is the set's own count"
    );
}

#[test]
fn an_empty_search_reports_zero_rather_than_no_answer() {
    let crit = SearchCriteria::up_to_year(YEAR);
    let m = finalize(
        &crit,
        None,
        &set_of(0),
        false,
        SparqlEndpoint::Qlever,
        YEAR,
        NOW,
    );

    assert_eq!(
        record_count(&m),
        Some(0),
        "an empty search reports zero, not an absent count"
    );
}
