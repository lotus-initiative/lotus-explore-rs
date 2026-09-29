// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::{FetchResult, PlannedResultsFetch};
use crate::features::explore::search_metrics::SearchMetrics;
use crate::features::explore::types::{DomainError, ParseFault, QueryStage};
use crate::perf;
use crate::repositories::LotusRepository;
use crate::repositories::RepositoryError;
use crate::services::search_telemetry as telemetry;
// Named at the use site rather than through the shared shims: only the wasm
// fetch path needs these, and re-exporting them from a shared module made them
// look unused on the native build, where this file does not exist.
use lotus_query::{counts_query, limit_query, parse_compounds_csv, parse_counts_csv};

pub(super) async fn fetch_results<R: LotusRepository>(
    repo: &R,
    plan: &PlannedResultsFetch<'_>,
    metrics: &mut SearchMetrics,
    on_processing: &impl Fn(),
) -> Result<FetchResult, DomainError> {
    let count_query = counts_query(plan.execution_query);
    let results_query = limit_query(plan.execution_query, plan.display_limit);

    // Display query FIRST, alone — it is authoritative and its failure fails the
    // search (subject to backoff). Fetching it alone guarantees it can never
    // race the COUNT, which is what `try_join!` did (the 15:00 burst that 429'd).
    //
    // Cache: reuse a previously fetched page (back/repeat navigation) instead of
    // re-hitting QLever. Keys come from `crate::cache_key::build_search_cache_key`,
    // the same keys the native server uses, so both cache paths stay compatible.
    let results_key =
        crate::cache_key::build_search_cache_key(plan.execution_query, plan.display_limit, true);
    let results_timer = perf::start_timer("LOTUS:results_page_query");
    let (results_csv, fetched_remote) = if let Some(cached) = crate::cache::get_cached(&results_key)
    {
        (cached, false)
    } else {
        let fetched = repo
            .sparql_body(&results_query)
            .await
            .map_err(DomainError::transport_at(QueryStage::ResultsQuery))?;
        crate::cache::store_cached(results_key, fetched.clone());
        (fetched, true)
    };
    let results_elapsed = perf::end_timer("LOTUS:results_page_query", results_timer);
    if fetched_remote {
        metrics.add_network(results_elapsed);
    }

    on_processing();

    let results_parse_timer = perf::start_timer("LOTUS:results_page_parse");
    let rows =
        parse_compounds_csv(&results_csv, plan.display_limit).map_err(results_csv_parse_error)?;
    let results_parse_elapsed = perf::end_timer("LOTUS:results_page_parse", results_parse_timer);
    metrics.add_parse(results_parse_elapsed);

    // COUNT query — back from QLever, but made safe for the anonymous quota:
    //  (a) fired strictly AFTER the display query succeeds (sequential, never
    //      `try_join!`), so it cannot create a burst by racing the display; and
    //  (b) best-effort — a 429 on it is swallowed (`None`) so it never fails or
    //      retry-amplifies the search. Once Qlever's window resets (no more
    //      burst), this returns the true total for the taxon.
    // `query_counts_from_base` is the "dumb pagination" COUNT(DISTINCT …) over
    // the base (incl. REFERENCE_METADATA_OPTIONAL/PROPERTIES_OPTIONAL, no LIMIT);
    // it is the heavy request that 429'd when fired concurrently with the
    // display query.
    let count_timer = perf::start_timer("LOTUS:results_count_query");
    let total_stats = repo
        .sparql_body(&count_query)
        .await
        .ok()
        .and_then(|c| parse_counts_csv(&c).ok());
    let count_elapsed = perf::end_timer("LOTUS:results_count_query", count_timer);
    if total_stats.is_some() {
        metrics.add_network(count_elapsed);
    }

    let total_matches = total_stats.as_ref().map(|s| s.n_entries);
    let display_capped_rows =
        total_matches.map_or(rows.len() >= plan.display_limit, |t| t > rows.len());
    telemetry::results_fetch_done(
        results_elapsed
            .saturating_add(count_elapsed)
            .saturating_add(results_parse_elapsed),
        rows.len(),
        total_matches.unwrap_or(rows.len()),
    );

    Ok(FetchResult {
        rows,
        total_stats,
        total_matches,
        display_capped_rows,
    })
}

pub(super) fn is_probable_memory_limit(err: &DomainError) -> bool {
    fn has_memory_signature(msg: &str) -> bool {
        let m = msg.to_ascii_lowercase();
        m.contains("out of memory")
            || m.contains("memory")
            || m.contains("too large")
            || m.contains("allocation")
            || m.contains("capacity")
    }

    match err {
        DomainError::Transport { source, .. } => match source {
            RepositoryError::NotConfigured => false,
            RepositoryError::Network(detail) | RepositoryError::Parse(detail) => {
                has_memory_signature(detail.as_ref())
            }
            RepositoryError::Http { body, .. } => has_memory_signature(body),
        },
        DomainError::Parse(ParseFault::ResultsCsv { details }) => has_memory_signature(details),
        _ => false,
    }
}

fn results_csv_parse_error(err: impl std::fmt::Display) -> DomainError {
    DomainError::Parse(ParseFault::ResultsCsv {
        details: err.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasm_preview_rows_are_bounded() {
        let csv = lotus_search::ResponseBody::from_static(
            b"compound,compoundLabel,taxon,ref_qid\nQ1,One,Q10,Q20\nQ2,Two,Q11,Q21\n",
        );

        let rows = parse_compounds_csv(&csv, 1).expect("display parse");
        assert_eq!(rows.len(), 1);
    }
}
