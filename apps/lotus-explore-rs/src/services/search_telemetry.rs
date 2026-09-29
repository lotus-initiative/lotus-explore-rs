// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Centralized telemetry/logging helpers for the Explore search pipeline.

use crate::utils::logging::{log_debug_evt, log_info_evt, log_timing_evt, log_warn_evt};
use std::time::Duration;

pub fn search_start() {
    log_info_evt("search", "start", "begin", None);
}

pub fn search_inflight_cancelled() {
    log_debug_evt("search", "start", "inflight_cancelled", None);
}

pub fn search_duplicate_suppressed() {
    log_debug_evt("search", "start", "duplicate_suppressed", None);
}

pub fn ignored_stale_result(request_token: u64) {
    log_debug_evt(
        "search",
        "finish",
        "stale_result_ignored",
        Some(&format!("request_token={request_token}")),
    );
}

pub fn ignored_stale_error(request_token: u64) {
    log_debug_evt(
        "search",
        "finish",
        "stale_error_ignored",
        Some(&format!("request_token={request_token}")),
    );
}

pub fn search_error_classified(error_class: &str, attempt: u32, will_retry: bool) {
    let state = if will_retry { "retryable" } else { "permanent" };
    log_info_evt(
        "search",
        "error_recovery",
        state,
        Some(&format!(
            "error_class={error_class} attempt={attempt} will_retry={will_retry}"
        )),
    );
}

pub fn search_retry_scheduled(error_class: &str, next_attempt: u32, backoff_ms: u64) {
    log_info_evt(
        "search",
        "error_recovery",
        "retry_scheduled",
        Some(&format!(
            "error_class={error_class} next_attempt={next_attempt} backoff_ms={backoff_ms}"
        )),
    );
}

pub fn search_max_retries_exceeded(error_class: &str, attempt: u32) {
    log_warn_evt(
        "search",
        "error_recovery",
        "max_retries_exceeded",
        Some(&format!("error_class={error_class} attempt={attempt}")),
    );
}

pub fn search_success_after_retries(retry_count: u32) {
    log_info_evt(
        "search",
        "error_recovery",
        "success_after_retries",
        Some(&format!("retry_count={retry_count}")),
    );
}

pub fn api_path_not_available(reason: &str) {
    log_info_evt("search", "api", "path_not_available", Some(reason));
}

pub fn record_api_success(elapsed: Duration, rows: usize, total_matches: usize) {
    log_timing_evt(
        "search",
        "api",
        "success",
        elapsed,
        Some(&format!("rows={rows} total_matches={total_matches}")),
    );
}

pub fn api_fallback_direct(elapsed: Duration, reason: &str) {
    log_timing_evt(
        "search",
        "api",
        "fallback_direct",
        elapsed,
        Some(&format!("reason={reason}")),
    );
}

pub fn direct_download_ready(elapsed: Duration) {
    log_timing_evt(
        "search",
        "direct_download",
        "ready",
        elapsed,
        Some("skipped=count_and_preview"),
    );
}

pub fn search_complete(elapsed: Duration, display_rows: usize, total_matches: usize) {
    log_timing_evt(
        "search",
        "complete",
        "done",
        elapsed,
        Some(&format!(
            "display_rows={display_rows} total_matches={total_matches}"
        )),
    );
}

pub fn search_summary_done(details: &str) {
    log_info_evt("search", "summary", "done", Some(details));
}

pub fn search_summary_slow_query(details: &str) {
    log_warn_evt("search", "summary", "slow_query", Some(details));
}

pub fn download_startup_unsupported_format(format: &str) {
    log_warn_evt(
        "download",
        "startup",
        "unsupported_format",
        Some(&format!("format={format}")),
    );
}

pub fn download_startup_auto_search_triggered(format: &str) {
    log_info_evt(
        "download",
        "startup",
        "auto_search_triggered",
        Some(&format!("format={format}")),
    );
}

pub fn search_startup_auto_search_execute() {
    log_info_evt(
        "search",
        "startup",
        "auto_search_triggered",
        Some("execute=true"),
    );
}

pub fn download_dispatch_waiting_loading(format: &str) {
    log_debug_evt(
        "download",
        "dispatch",
        "waiting_loading",
        Some(&format!("format={format}")),
    );
}

pub fn download_dispatch_waiting_query(format: &str) {
    log_debug_evt(
        "download",
        "dispatch",
        "waiting_query",
        Some(&format!("format={format}")),
    );
}

pub fn download_startup_dispatch_query_check(
    format: &str,
    has_service: bool,
    has_select: bool,
    query_bytes: usize,
) {
    log_debug_evt(
        "download",
        "startup_dispatch",
        "query_check",
        Some(&format!(
            "format={format} has_SERVICE={has_service} has_SELECT={has_select} query_bytes={query_bytes}"
        )),
    );
}

pub fn download_dispatch_started(format: &str) {
    log_info_evt(
        "download",
        "dispatch",
        "started",
        Some(&format!("format={format}")),
    );
}

pub fn download_dispatch_error(format: &str, reason: &str) {
    log_warn_evt(
        "download",
        "dispatch",
        "error",
        Some(&format!("format={format} reason={reason}")),
    );
}

pub fn query_build_sachem_query_created(has_service: bool) {
    log_debug_evt(
        "search",
        "query_build",
        "sachem_query_created",
        Some(&format!("has_SERVICE={has_service}")),
    );
}

pub fn query_build_after_server_filters(has_service: bool, has_filter: bool) {
    log_debug_evt(
        "search",
        "query_build",
        "after_server_filters",
        Some(&format!(
            "has_SERVICE={has_service} has_FILTER={has_filter}"
        )),
    );
}

pub fn results_fetch_started(display_limit: usize) {
    log_debug_evt(
        "search",
        "fetching_results",
        "started",
        Some(&format!("display_limit={display_limit}")),
    );
}

pub fn results_fetch_done(elapsed: Duration, rows: usize, total_matches: usize) {
    log_timing_evt(
        "search",
        "fetching_results",
        "done",
        elapsed,
        Some(&format!("rows={rows} total_matches={total_matches}")),
    );
}

pub fn taxon_cache_hit(elapsed: Duration, taxon_input: &str, qid: &str) {
    log_timing_evt(
        "search",
        "resolving_taxon",
        "cache_hit",
        elapsed,
        Some(&format!("taxon_input={taxon_input} qid={qid}")),
    );
}

pub fn taxon_sparql_done(elapsed: Duration) {
    log_timing_evt("search", "resolving_taxon", "sparql_done", elapsed, None);
}
