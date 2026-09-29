// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![allow(clippy::future_not_send)]

use super::{FetchResult, PlannedResultsFetch};
use crate::features::explore::search_metrics::SearchMetrics;
use crate::features::explore::types::{DomainError, ParseFault, QueryStage};
use crate::models::{CompoundEntry, DatasetStats};
use crate::perf;
use crate::repositories::LotusRepository;
use crate::services::search_telemetry as telemetry;
// Named here because only the native fetch path streams a file; going through the
// shared shims made it look unused on the wasm build.
use lotus_query::parse_compounds_stream;
use std::io::{BufReader, Seek};
use std::time::Duration;

struct FetchedResultsCsv {
    payload: FetchedResultsPayload,
    network_elapsed: Duration,
}

enum FetchedResultsPayload {
    TempFile(tempfile::NamedTempFile),
}

struct ProcessedResults {
    rows: Vec<CompoundEntry>,
    total_stats: DatasetStats,
    total_matches: usize,
    display_capped_rows: bool,
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

    let processed = process_full_results_csv(fetched.payload, plan.display_limit)?;
    metrics.add_parse(processed.parse_elapsed);
    telemetry::results_fetch_done(
        fetched
            .network_elapsed
            .saturating_add(processed.parse_elapsed),
        processed.rows.len(),
        processed.total_matches,
    );

    Ok(FetchResult {
        rows: processed.rows,
        total_stats: Some(processed.total_stats),
        total_matches: Some(processed.total_matches),
        display_capped_rows: processed.display_capped_rows,
    })
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

fn process_full_results_csv(
    payload: FetchedResultsPayload,
    display_limit: usize,
) -> Result<ProcessedResults, DomainError> {
    let parse_timer = perf::start_timer("LOTUS:results_parse");
    let (rows, total_stats, parse_capped) = match payload {
        FetchedResultsPayload::TempFile(mut file) => {
            file.as_file_mut().rewind().map_err(|e| {
                DomainError::Parse(ParseFault::ResultsCsv {
                    details: format!("tempfile rewind failed: {e}"),
                })
            })?;
            parse_compounds_stream(BufReader::new(file.as_file_mut()), display_limit)
        }
    }
    .map_err(results_csv_parse_error)?;
    let parse_elapsed = perf::end_timer("LOTUS:results_parse", parse_timer);

    let total_matches = total_stats.n_entries;
    let display_capped_rows = parse_capped || total_matches > rows.len();
    Ok(ProcessedResults {
        rows,
        total_stats,
        total_matches,
        display_capped_rows,
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
    fn process_stage_marks_capped_and_preserves_full_counts() {
        let payload = {
            use std::io::Write;

            let mut file = tempfile::NamedTempFile::new().expect("tempfile create");
            file.write_all(
                b"compound,compoundLabel,taxon,ref_qid\nhttp://www.wikidata.org/entity/Q1,One,http://www.wikidata.org/entity/Q10,http://www.wikidata.org/entity/Q20\nhttp://www.wikidata.org/entity/Q2,Two,http://www.wikidata.org/entity/Q11,http://www.wikidata.org/entity/Q21\n",
            )
            .expect("tempfile write");
            FetchedResultsPayload::TempFile(file)
        };

        let processed = process_full_results_csv(payload, 1).expect("csv should parse");
        assert_eq!(processed.rows.len(), 1);
        assert_eq!(processed.total_matches, 2);
        assert_eq!(processed.total_stats.n_entries, 2);
        assert!(processed.display_capped_rows);
    }
}
