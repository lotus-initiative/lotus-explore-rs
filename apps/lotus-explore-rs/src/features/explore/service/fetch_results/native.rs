// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
// The search future is not `Send`: `on_phase` is a closure over a Dioxus
// `Signal`, which is a `RefCell`. See the full explanation in
// `features/explore/executor.rs`, which is the same cause reached from here.
#![expect(
    clippy::future_not_send,
    reason = "`on_phase` captures a Dioxus `Signal`, which is a `RefCell` and not `Sync`"
)]

use super::{FetchResult, PlannedResultsFetch};
use crate::features::explore::search_metrics::SearchMetrics;
use crate::features::explore::types::{DomainError, ParseFault, QueryStage};
use crate::perf;
use crate::repositories::LotusRepository;
use crate::services::search_telemetry as telemetry;
use lotus_model::ColumnarResultSet;
// Named here because only the native fetch path streams a file; going through the
// shared shims made it look unused on the wasm build.
use lotus_query::parse_compounds_columnar;
use std::io::{BufReader, Seek};
use std::sync::Arc;
use std::time::Duration;

struct FetchedResultsCsv {
    payload: FetchedResultsPayload,
    network_elapsed: Duration,
}

enum FetchedResultsPayload {
    TempFile(tempfile::NamedTempFile),
}

struct ProcessedResults {
    set: Arc<ColumnarResultSet>,
    parse_elapsed: Duration,
}

pub(super) async fn fetch_results<R: LotusRepository>(
    repo: &R,
    plan: &PlannedResultsFetch<'_>,
    metrics: &mut SearchMetrics,
    on_processing: &impl Fn(),
) -> Result<FetchResult, DomainError> {
    let fetched = fetch_results_csv(repo, plan).await?;
    metrics.add_network(fetched.network_elapsed);
    on_processing();

    let processed = process_full_results_csv(fetched.payload)?;
    metrics.add_parse(processed.parse_elapsed);

    let stats = processed.set.stats();
    telemetry::results_fetch_done(
        fetched
            .network_elapsed
            .saturating_add(processed.parse_elapsed),
        stats.n_entries,
        stats.n_entries,
    );
    Ok(FetchResult::from_set(processed.set))
}

async fn fetch_results_csv<R: LotusRepository>(
    repo: &R,
    plan: &PlannedResultsFetch<'_>,
) -> Result<FetchedResultsCsv, DomainError> {
    let results_timer = perf::start_timer("LOTUS:results_query");
    let payload = FetchedResultsPayload::TempFile(
        repo.sparql_tempfile(plan.execution_query)
            .await
            .map_err(DomainError::transport_at(QueryStage::ResultsQuery))?,
    );
    let network_elapsed = perf::end_timer("LOTUS:results_query", results_timer);
    Ok(FetchedResultsCsv {
        payload,
        network_elapsed,
    })
}

/// Fold the spooled body into a columnar set.
///
/// Spooling to a file keeps a large result off the heap while the endpoint is
/// still answering. Reading it back through the incremental CSV reader makes the
/// peak the finished set, not the set plus the whole payload.
fn process_full_results_csv(
    payload: FetchedResultsPayload,
) -> Result<ProcessedResults, DomainError> {
    let parse_timer = perf::start_timer("LOTUS:results_parse");
    let set = match payload {
        FetchedResultsPayload::TempFile(mut file) => {
            // The spool left the cursor at the end of what it wrote, and reading
            // through a handle that shares that cursor would find nothing.
            file.as_file_mut().rewind().map_err(|e| {
                DomainError::Parse(ParseFault::ResultsCsv {
                    details: format!("tempfile rewind failed: {e}"),
                })
            })?;
            parse_compounds_columnar(BufReader::new(file.as_file_mut()))
        }
    }
    .map_err(results_csv_parse_error)?;
    let parse_elapsed = perf::end_timer("LOTUS:results_parse", parse_timer);

    Ok(ProcessedResults {
        set: Arc::new(set),
        parse_elapsed,
    })
}

fn results_csv_parse_error(err: impl std::fmt::Display) -> DomainError {
    DomainError::Parse(ParseFault::ResultsCsv {
        details: err.to_string(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn the_whole_body_becomes_a_set_with_exact_counts() {
        let payload = {
            use std::io::Write;

            let mut file = tempfile::NamedTempFile::new().expect("tempfile create");
            file.write_all(
                b"compound,compoundLabel,taxon,ref_qid\nhttp://www.wikidata.org/entity/Q1,One,http://www.wikidata.org/entity/Q10,http://www.wikidata.org/entity/Q20\nhttp://www.wikidata.org/entity/Q2,Two,http://www.wikidata.org/entity/Q11,http://www.wikidata.org/entity/Q21\n",
            )
            .expect("tempfile write");
            FetchedResultsPayload::TempFile(file)
        };

        let processed = process_full_results_csv(payload).expect("csv should parse");
        let stats = processed.set.stats();

        assert_eq!(
            processed.set.row_count(),
            2,
            "both rows are kept: nothing caps the native path either"
        );
        assert_eq!(stats.n_entries, 2);
        assert_eq!(stats.n_entries_unique, 2);
        assert_eq!(stats.n_compounds, 2);
    }
}
