// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! `QLever`-to-WDQS fallback orchestration, HTTP send/status handling, and timeout detection.

use std::ops::AsyncFn;

use lotus_query::FallbackService;

use super::budget::{form_body, query_budget, request_headers};
use super::endpoints::{Answer, Endpoint, Service, StreamAnswer};
use super::error_format::compact;
use super::retry::{execute, execute_streaming};
use crate::client::{Http, HttpResponse};
use crate::error::{FetchError, ResponseFormat};

/// Run `query` against `QLever`, falling back to WDQS when the endpoint is
/// unreachable.
///
/// A 4xx is *not* a reason to fall back: WDQS would reject the same query, and
/// trying would only double the load on a query that is wrong.
///
/// # Errors
/// Returns [`FetchError`] from either endpoint, and the WDQS failure if
/// `QLever` was unreachable and WDQS then failed too.
pub async fn execute_with_fallback<H: Http>(
    http: &H,
    query: &str,
    format: ResponseFormat,
) -> Result<Answer, FetchError> {
    with_fallback(http, query, format, execute).await
}

/// Run `query` against `QLever` and stream the body, falling back to WDQS when
/// the endpoint is unreachable.
///
/// # Errors
/// Returns [`FetchError`] from either endpoint, and the WDQS failure if `QLever`
/// was unreachable and WDQS then failed too.
pub async fn execute_streaming_with_fallback<H: Http>(
    http: &H,
    query: &str,
    format: ResponseFormat,
) -> Result<StreamAnswer, FetchError> {
    with_fallback(http, query, format, execute_streaming).await
}

/// What to try when the primary endpoint is unreachable: the WDQS endpoint the
/// query maps to, with the query rewritten for it.
pub(super) fn fallback_endpoint(err: &FetchError, query: &str) -> Option<(Endpoint, String)> {
    if !err.is_endpoint_unavailable() {
        return None;
    }
    let (service, rewritten) = lotus_query::wdqs_fallback(query);
    let service = match service {
        FallbackService::Scholarly => Service::Scholarly,
        FallbackService::Main => Service::Wdqs,
    };
    Some((Endpoint::new(service), rewritten))
}

async fn with_fallback<H, T, Run>(
    http: &H,
    query: &str,
    format: ResponseFormat,
    run: Run,
) -> Result<T, FetchError>
where
    H: Http,
    Run: AsyncFn(&H, Endpoint, &str, ResponseFormat) -> Result<T, FetchError>,
{
    match run(http, Endpoint::new(Service::Qlever), query, format).await {
        Ok(answer) => Ok(answer),
        Err(err) => match fallback_endpoint(&err, query) {
            Some((endpoint, rewritten)) => run(http, endpoint, &rewritten, format).await,
            None => Err(err),
        },
    }
}

/// GET `url`, for a prepared export URL that already carries its format.
///
/// # Errors
/// Returns [`FetchError`] if the URL could not be fetched or the server answered
/// with a non-2xx status. There is no retry: a prepared export URL is
/// idempotent, and the caller can decide.
pub async fn fetch_url<H: Http>(
    http: &H,
    url: &str,
    format: ResponseFormat,
) -> Result<Vec<u8>, FetchError> {
    let response = http.get(url, format.accept()).await?;
    let status = response.status();
    let body = response.bytes().await?;

    if !is_success(status) {
        return Err(FetchError::Http {
            status,
            message: compact(&body),
        });
    }
    Ok(body.to_vec())
}

pub(super) async fn send<H: Http>(
    http: &H,
    endpoint: &str,
    query: &str,
    format: ResponseFormat,
) -> Result<Vec<u8>, FetchError> {
    let bytes = send_response(http, endpoint, query, format)
        .await?
        .bytes()
        .await?;
    if bytes.is_empty() {
        return Err(FetchError::Empty);
    }
    Ok(bytes.to_vec())
}

/// POST `query` and hand back the response without reading its body.
pub(super) async fn send_response<H: Http>(
    http: &H,
    endpoint: &str,
    query: &str,
    format: ResponseFormat,
) -> Result<H::Response, FetchError> {
    let body = form_body(endpoint, query);
    let headers = request_headers(endpoint);
    let response = http
        .post_form(endpoint, format.accept(), body, &headers)
        .await?;

    let status = response.status();
    if is_success(status) {
        Ok(response)
    } else {
        // Read the body before reporting, because it carries the endpoint's
        // explanation and a gateway's is an HTML page.
        let response = response.bytes().await.map_err(|_| FetchError::Empty);
        let wdqs_timeout = response
            .as_ref()
            .is_ok_and(|bytes| is_wdqs_query_timeout(status, bytes));
        let message = response.map_or_else(|_| String::new(), |bytes| compact(&bytes));
        if is_qlever_query_timeout(endpoint, status, &message) || wdqs_timeout {
            return Err(FetchError::TimedOut {
                budget: query_budget(endpoint),
                message,
            });
        }
        Err(FetchError::Http { status, message })
    }
}

/// Whether a `429` came from `QLever` cancelling a query that outran its time
/// budget rather than reverse-proxy / REST Gateway rate limiting.
pub(super) fn is_qlever_query_timeout(endpoint: &str, status: u16, message: &str) -> bool {
    status == 429 && super::budget::is_qlever_host(endpoint) && !is_rate_limit_message(message)
}

fn is_rate_limit_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("too many") || lower.contains("rate limit")
}

/// Whether an error body is WDQS reporting that the query outran its time limit.
pub(super) fn is_wdqs_query_timeout(status: u16, body: &[u8]) -> bool {
    const BLAZEGRAPH_TIMEOUT: &[u8] = b"java.util.concurrent.TimeoutException";
    const PROXY_GATEWAY_TIMEOUT: &[u8] = b"gateway_timeout";
    const PROXY_BACKEND_TIMEOUT: &[u8] = b"backend_timeout";
    (status >= 500 && has_bytes(body, BLAZEGRAPH_TIMEOUT))
        || (status == 504
            && (has_bytes(body, PROXY_GATEWAY_TIMEOUT) || has_bytes(body, PROXY_BACKEND_TIMEOUT)))
}

fn has_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

pub(super) const fn is_success(status: u16) -> bool {
    status >= 200 && status < 300
}
