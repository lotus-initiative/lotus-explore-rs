// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! [`HybridRepository`] — the production `LotusRepository` implementation.

use crate::api;
use crate::api::SearchResponse;
use crate::repositories::{LotusRepository, RepositoryError};
use crate::sparql::transform_query_for_wdqs;
use crate::sparql::{self, FetchError, ResponseFormat, WDQS_WIKIDATA};
use lotus_model::ColumnarResultSet;
use lotus_search::SearchCriteria;
use std::cell::RefCell;

thread_local! {
    /// Tracks whether WDQS fallback was used for the current search.
    /// Reset at the start of each search operation.
    static WDQS_FALLBACK_USED: RefCell<bool> = const { RefCell::new(false) };
    /// Stores the WDQS-transformed query when fallback occurs.
    static WDQS_TRANSFORMED_QUERY: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Check if WDQS fallback was used in the current operation.
pub fn is_wdqs_fallback_used() -> bool {
    WDQS_FALLBACK_USED.with(|b| *b.borrow())
}

/// Get the WDQS-transformed query (if fallback occurred).
/// Strips any trailing LIMIT clause so the stored query is suitable for
/// downloads (which should not be capped at the interactive display limit).
pub fn get_wdqs_transformed_query() -> Option<String> {
    WDQS_TRANSFORMED_QUERY.with(|b| b.borrow().as_ref().map(|q| strip_limit_clause(q)))
}

/// Remove a trailing `LIMIT nnn` (case-insensitive) clause from a query string.
fn strip_limit_clause(query: &str) -> String {
    // Strip a trailing `LIMIT nnn` clause so downloads aren't capped at
    // a client-side row limit (there is no longer one on the interactive path).
    let trimmed = query.trim_end();
    let lower = trimmed.to_ascii_lowercase();
    // Find the last occurrence of "limit" preceded by whitespace or start of string
    let mut best_pos = None;
    for (i, _) in lower.match_indices("limit") {
        let preceded_ok = i == 0 || trimmed[..i].chars().last().is_some_and(char::is_whitespace);
        if !preceded_ok {
            continue;
        }
        let after = trimmed[i + 5..].trim_start();
        let num = after.split_whitespace().next().unwrap_or("");
        if !num.is_empty()
            && num.chars().all(|c| c.is_ascii_digit())
            && after[num.len()..].trim().is_empty()
        {
            best_pos = Some(i);
        }
    }
    best_pos.map_or_else(
        || query.to_string(),
        |pos| trimmed[..pos].trim_end().to_string(),
    )
}

/// Reset the WDQS fallback flag at the start of a new operation.
pub fn reset_wdqs_fallback_flag() {
    WDQS_FALLBACK_USED.with(|b| *b.borrow_mut() = false);
    WDQS_TRANSFORMED_QUERY.with(|b| *b.borrow_mut() = None);
}

/// Mark that WDQS fallback was used and store the transformed query.
/// Only stores the first transformed query (results query priority over count query).
fn mark_wdqs_fallback_used(query: String) {
    // Only store the first transformed query (results query priority over count query)
    WDQS_TRANSFORMED_QUERY.with(|transformed| {
        if transformed.borrow().is_none() {
            *transformed.borrow_mut() = Some(query);
        }
    });
    WDQS_FALLBACK_USED.with(|flag| {
        *flag.borrow_mut() = true;
    });
}

/// Rewrite `query` for the WDQS endpoint and record that WDQS fallback applied.
/// This is the single source of truth for the "prepare a query for WDQS
/// fallback" step, shared by the body / tempfile execution paths.
fn prepare_wdqs_fallback_query(query: &str) -> String {
    let wdqs_query = transform_query_for_wdqs(query);
    mark_wdqs_fallback_used(wdqs_query.clone());
    wdqs_query
}

/// Zero-size, `Copy` production repository.
/// Holds no state of its own; all configuration is read from environment and
/// runtime globals (`api_base_url`, `sparql::execute_sparql_body`, etc.).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct HybridRepository;

impl LotusRepository for HybridRepository {
    async fn api_search(
        &self,
        criteria: &SearchCriteria,
        limit: usize,
        include_counts: bool,
    ) -> Option<Result<SearchResponse, RepositoryError>> {
        // The API fast-path is opt-in: treat an empty (auto-detected dev) base URL
        // the same as "not configured" so we never build a malformed relative URL.
        if api::api_base_url().is_none_or(|b| b.is_empty()) {
            return Some(Err(RepositoryError::NotConfigured));
        }
        // Call the transport client directly, mapping ApiClientError → RepositoryError
        // via the existing `From` implementation.  Bypassing the ApiLayer / AppError
        // intermediary eliminates a 4-hop conversion chain with no semantic benefit.
        Some(
            api::search(criteria, limit, include_counts)
                .await
                .map_err(RepositoryError::from),
        )
    }

    async fn sparql_body(
        &self,
        query: &str,
    ) -> Result<lotus_search::ResponseBody, RepositoryError> {
        match with_rate_limit_backoff(|| sparql::execute_sparql_body(query)).await {
            Err(err) if is_qlever_unavailable(&err) => {
                log::warn!("event=qlever_unavailable action=fallback_wdqs_scholarly");
                let wdqs_query = prepare_wdqs_fallback_query(query);
                sparql::execute_sparql_body_at(&wdqs_query, WDQS_WIKIDATA)
                    .await
                    .map_err(map_fetch_error)
            }
            result => result.map_err(map_fetch_error),
        }
    }

    async fn sparql_columnar<F>(
        &self,
        query: &str,
        on_progress: &mut F,
    ) -> Result<ColumnarResultSet, RepositoryError>
    where
        F: FnMut(lotus_search::StreamProgress) + ?Sized,
    {
        if !should_use_split_occurrences(query) {
            match sparql_columnar_single(query, on_progress).await {
                Ok(set) => return Ok(set),
                Err(err) if is_timeout_or_truncation(&err) => {
                    log::warn!("event=single_query_truncated action=fallback_split_occurrences");
                }
                Err(err) => return Err(err),
            }
        }
        sparql_columnar_split(query, on_progress).await
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn sparql_tempfile(
        &self,
        query: &str,
    ) -> Result<tempfile::NamedTempFile, RepositoryError> {
        match with_rate_limit_backoff(|| sparql::execute_sparql_tempfile(query)).await {
            Err(err) if is_qlever_unavailable(&err) => {
                log::warn!("event=qlever_unavailable action=fallback_wdqs_scholarly");
                let wdqs_query = prepare_wdqs_fallback_query(query);
                sparql::execute_sparql_tempfile_at(&wdqs_query, WDQS_WIKIDATA)
                    .await
                    .map_err(map_fetch_error)
            }
            result => result.map_err(map_fetch_error),
        }
    }
}

/// High-level taxonomic clades whose unnarrowed subtrees in LOTUS are large
/// enough that streaming all 15 denormalized columns in a single response is
/// orders of magnitude slower on `QLever` than normalized occurrence +
/// dictionary streams.
const BROAD_TAXON_QIDS: &[&str] = &[
    "Q2382443",  // Biota
    "Q19088",    // Eukaryota
    "Q756",      // Plantae
    "Q11905549", // Viridiplantae
    "Q133527",   // Streptophyta
    "Q133524",   // Streptophytina
    "Q192154",   // Embryophyta
    "Q131221",   // Tracheophytes
    "Q1492103",  // Euphyllophyta
    "Q25814",    // Spermatophyta
    "Q25314",    // Magnoliopsida
    "Q169458",   // Angiosperms
    "Q148604",   // Mesangiospermae
    "Q1363010",  // Pentapetalae
    "Q165468",   // eudicots
    "Q165479",   // core eudicots
    "Q165458",   // asterids
    "Q1363022",  // campanulids
    "Q1363017",  // lamiids
    "Q165463",   // rosids
    "Q2697791",  // fabids
    "Q2697795",  // malvids
    "Q134267",   // Liliopsida / monocots
    "Q25309",    // Magnoliids
    "Q132089",   // Pinophyta / Acrogymnospermae
    "Q28499",    // Asterales
    "Q25400",    // Asteraceae
    "Q21754",    // Gentianales
    "Q155900",   // Apocynaceae
    "Q156569",   // Rubiaceae
    "Q21878",    // Fabales
    "Q44448",    // Fabaceae
    "Q21887",    // Lamiales
    "Q53476",    // Lamiaceae
    "Q26336",    // Sapindales
    "Q146039",   // Rutaceae
    "Q158438",   // Meliaceae
    "Q27525",    // Malpighiales
    "Q156584",   // Euphorbiaceae
    "Q21111",    // Apiales
    "Q145809",   // Apiaceae
    "Q21809",    // Solanales
    "Q134172",   // Solanaceae
    "Q28505",    // Poales
    "Q43238",    // Poaceae
    "Q27207",    // Ranunculales
    "Q21861",    // Myrtales
    "Q21897",    // Ericales
    "Q21856",    // Caryophyllales
    "Q26229",    // Asparagales
    "Q21882",    // Rosales
    "Q27323",    // Brassicales
    "Q764",      // Fungi
    "Q174645",   // Ascomycota
    "Q174663",   // Basidiomycota
    "Q729",      // Animalia
    "Q10915",    // Chordata
    "Q1360",     // Arthropoda
    "Q5184",     // Porifera
    "Q25441",    // Cnidaria
    "Q10876",    // Bacteria
    "Q131144",   // Actinomycetota
    "Q93315",    // Cyanobacteria
];

fn contains_taxon_qid(query: &str, qid: &str) -> bool {
    let token = format!("wd:{qid}");
    query.match_indices(&token).any(|(pos, _)| {
        query
            .as_bytes()
            .get(pos + token.len())
            .is_none_or(|b| !b.is_ascii_alphanumeric())
    })
}

fn is_unnarrowed_query(query: &str) -> bool {
    !(query.contains("SERVICE idsm:wikidata")
        || query.contains("VALUES ?c {")
        || query.contains("VALUES ?r {")
        || query.contains("FILTER(BOUND(?compound_formula))")
        || query.contains("FILTER(?compound_mass")
        || query.contains("FILTER(BOUND(?ref_date)"))
}

/// True when `query` is an unconstrained whole-graph scan or an unnarrowed
/// broad-clade query that should use normalized occurrence + dictionary streams
/// rather than a single 15-column query.
fn needs_split_occurrences(query: &str) -> bool {
    if !is_unnarrowed_query(query) {
        return false;
    }
    if !query.contains("wdt:P171*") {
        return true;
    }
    BROAD_TAXON_QIDS
        .iter()
        .any(|qid| contains_taxon_qid(query, qid))
}

fn should_use_split_occurrences(query: &str) -> bool {
    // Only the browser keeps a seed set, so only it can tell an unnarrowed query
    // is broad from the cache alone.
    #[cfg(target_arch = "wasm32")]
    let seeded_broad = is_unnarrowed_query(query) && crate::cache::has_broad_seed_set();
    #[cfg(not(target_arch = "wasm32"))]
    let seeded_broad = false;
    needs_split_occurrences(query) || seeded_broad
}

/// Backoff delays (in milliseconds) for in-place retries when `qlever.dev`'s
/// reverse proxy returns `429 Too Many Requests`.
const RATE_LIMIT_BACKOFFS_MS: &[u32] = &[2_000, 4_000, 6_000];

/// True when a `429` came from reverse-proxy request-rate throttling
/// (`429 Too Many Requests`) rather than `QLever` cancelling a query that
/// exceeded its 30s execution budget (`Operation timed out`).
fn is_rate_limit_refusal(err: &FetchError) -> bool {
    match err {
        FetchError::TimedOut { message, .. }
        | FetchError::Http {
            status: 429,
            message,
        } => !message.contains("timed out"),
        _ => false,
    }
}

#[allow(clippy::unused_async)]
async fn sleep_ms(ms: u32) {
    #[cfg(target_arch = "wasm32")]
    gloo_timers::future::TimeoutFuture::new(ms).await;
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::sleep(std::time::Duration::from_millis(u64::from(ms)));
}

async fn with_rate_limit_backoff<T, F, Fut>(mut op: F) -> Result<T, FetchError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, FetchError>>,
{
    for &delay_ms in RATE_LIMIT_BACKOFFS_MS {
        match op().await {
            Err(err) if is_rate_limit_refusal(&err) => {
                log::warn!("event=qlever_rate_limited action=in_place_retry backoff_ms={delay_ms}");
                sleep_ms(delay_ms).await;
            }
            result => return result,
        }
    }
    op().await
}

fn is_timeout_or_truncation(err: &RepositoryError) -> bool {
    matches!(
        lotus_explore_core::explore::transport_classification::classify_transport_error(err),
        lotus_explore_core::explore::transport_classification::TransportFailureKind::QueryTooExpensive
            | lotus_explore_core::explore::transport_classification::TransportFailureKind::Truncated
    )
}

async fn open_chunks_with_wdqs_fallback(
    qlever_query: &str,
    full_query: &str,
) -> Result<lotus_search::ChunkedBody, RepositoryError> {
    match with_rate_limit_backoff(|| sparql::execute_sparql_chunks(qlever_query)).await {
        Ok(chunks) => Ok(chunks),
        Err(err) if is_qlever_unavailable(&err) => {
            log::warn!("event=qlever_unavailable action=fallback_wdqs_scholarly");
            let wdqs_query = prepare_wdqs_fallback_query(full_query);
            sparql::execute_sparql_chunks_at(&wdqs_query, WDQS_WIKIDATA, ResponseFormat::Csv)
                .await
                .map_err(map_fetch_error)
        }
        Err(err) => Err(map_fetch_error(err)),
    }
}

async fn sparql_columnar_single<F>(
    query: &str,
    on_progress: &mut F,
) -> Result<ColumnarResultSet, RepositoryError>
where
    F: FnMut(lotus_search::StreamProgress) + ?Sized,
{
    let chunks = open_chunks_with_wdqs_fallback(query, query).await?;
    lotus_search::columnar_from_chunks_reporting(chunks, on_progress)
        .await
        .map_err(map_fetch_error)
}

async fn sparql_columnar_split<F>(
    query: &str,
    on_progress: &mut F,
) -> Result<ColumnarResultSet, RepositoryError>
where
    F: FnMut(lotus_search::StreamProgress) + ?Sized,
{
    let occ_query = lotus_query::occurrences_query(query);
    let mut chunks = open_chunks_with_wdqs_fallback(&occ_query, query).await?;
    let mut reader = lotus_query::CsvColumnarReader::new();
    lotus_search::feed_chunks_reporting(&mut reader, &mut chunks, on_progress)
        .await
        .map_err(map_fetch_error)?;
    // Emit a boundary report with unchanged `bytes` so the caller can flush
    // the exact final occurrence count and switch the loading phase before
    // dictionary enrichment starts.
    on_progress(lotus_search::StreamProgress {
        rows: reader.row_count(),
        bytes: reader.byte_count(),
    });
    #[cfg(target_arch = "wasm32")]
    {
        gloo_timers::future::TimeoutFuture::new(0).await;
        crate::cache::seed_reader_from_cache(&mut reader);
    }
    lotus_search::enrich_columnar_reader(
        &mut reader,
        |dict_query| async move {
            with_rate_limit_backoff(|| sparql::execute_sparql_chunks(&dict_query)).await
        },
        &mut |_| {},
    )
    .await
    .map_err(map_fetch_error)?;
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(extra) = reader.take_extra_entities() {
            crate::cache::store_extra_seed_set(std::sync::Arc::new(extra));
        }
        gloo_timers::future::TimeoutFuture::new(0).await;
    }
    reader
        .finish()
        .map_err(|e| RepositoryError::parse(e.to_string()))
}

/// True when `QLever` is unavailable and the query should be retried against WDQS.
/// The transport layer already retries transient failures, so reaching this layer means the
/// in-endpoint attempts were exhausted.
fn is_qlever_unavailable(err: &FetchError) -> bool {
    matches!(
        err,
        FetchError::Http { status: 502, .. } | FetchError::Network(_)
    )
}

fn map_fetch_error(err: FetchError) -> RepositoryError {
    match err {
        FetchError::Http { status, message } => RepositoryError::Http {
            status,
            body: message,
        },
        FetchError::Network(msg) => RepositoryError::network(msg),
        FetchError::Parse(msg) => RepositoryError::parse(msg),
        // Deliberately not a network error. `is_qlever_unavailable` reads
        // `Network` as "the endpoint is down, try WDQS", and a body that stopped
        // half way says nothing about whether the endpoint is reachable.
        FetchError::Truncated { .. } => RepositoryError::truncated(err.to_string()),
        // Not a network error either, and the same reason: a query QLever
        // cancelled for running over its time limit is a query `WDQS` would also
        // have to run, so falling back would answer the same expensive question
        // twice on two endpoints. It is a 429 on the wire, which is what the
        // caller sees.
        FetchError::TimedOut { .. } => RepositoryError::Http {
            status: 429,
            body: err.to_string(),
        },
        FetchError::Empty => RepositoryError::parse("query returned no results"),
    }
}

#[cfg(test)]
#[path = "hybrid/tests.rs"]
mod tests;
