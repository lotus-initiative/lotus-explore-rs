// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! [`HybridRepository`] — the production `LotusRepository` implementation.

use crate::api;
use crate::api::SearchResponse;
use crate::repositories::{LotusRepository, RepositoryError};
use crate::sparql::transform_query_for_wdqs;
use crate::sparql::{self, FetchError, WDQS_WIKIDATA};
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
    // the interactive display limit (e.g. 500 from runtime_table_row_limit()).
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
        match sparql::execute_sparql_body(query).await {
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

    #[cfg(not(target_arch = "wasm32"))]
    async fn sparql_tempfile(
        &self,
        query: &str,
    ) -> Result<tempfile::NamedTempFile, RepositoryError> {
        match sparql::execute_sparql_tempfile(query).await {
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
        FetchError::Empty => RepositoryError::parse("query returned no results"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_limit_removes_trailing_limit() {
        let q = "SELECT ?s WHERE { ?s ?p ?o } LIMIT 500";
        assert_eq!(strip_limit_clause(q), "SELECT ?s WHERE { ?s ?p ?o }");
    }

    #[test]
    fn strip_limit_preserves_query_without_limit() {
        let q = "SELECT ?s WHERE { ?s ?p ?o }";
        assert_eq!(strip_limit_clause(q), "SELECT ?s WHERE { ?s ?p ?o }");
    }

    #[test]
    fn strip_limit_preserves_inner_limit() {
        let q = "SELECT ?s WHERE { { SELECT ?s WHERE { ?s ?p ?o } LIMIT 10 } LIMIT 500";
        // Should only strip the last LIMIT
        assert_eq!(
            strip_limit_clause(q),
            "SELECT ?s WHERE { { SELECT ?s WHERE { ?s ?p ?o } LIMIT 10 }"
        );
    }

    #[test]
    fn strip_limit_handles_case_insensitive() {
        let q = "SELECT ?s WHERE { ?s ?p ?o } limit 500";
        assert_eq!(strip_limit_clause(q), "SELECT ?s WHERE { ?s ?p ?o }");
    }

    #[test]
    fn strip_limit_handles_newline() {
        let q = "SELECT ?s WHERE { ?s ?p ?o }\nLIMIT 500";
        assert_eq!(strip_limit_clause(q), "SELECT ?s WHERE { ?s ?p ?o }");
    }

    #[test]
    fn map_fetch_error_preserves_http_status_and_body() {
        let mapped = map_fetch_error(FetchError::Http {
            status: 400,
            message: "invalid query".to_string(),
        });
        assert_eq!(
            mapped,
            RepositoryError::Http {
                status: 400,
                body: "invalid query".to_string(),
            }
        );
    }

    #[test]
    fn map_fetch_error_keeps_network_as_network() {
        let mapped = map_fetch_error(FetchError::Network("timeout".to_string()));
        assert!(matches!(mapped, RepositoryError::Network(_)));
    }

    #[test]
    fn qlever_unavailability_includes_502_and_network_failures() {
        let gateway = FetchError::Http {
            status: 502,
            message: "upstream gateway error (HTML payload)".into(),
        };
        assert!(is_qlever_unavailable(&gateway));
        assert!(is_qlever_unavailable(&FetchError::Network(
            "connection failed".into()
        )));
        assert!(!is_qlever_unavailable(&FetchError::Http {
            status: 500,
            message: "boom".into()
        }));
        assert!(!is_qlever_unavailable(&FetchError::Http {
            status: 400,
            message: "bad query".into()
        }));
        assert!(!is_qlever_unavailable(&FetchError::Empty));
    }
}
