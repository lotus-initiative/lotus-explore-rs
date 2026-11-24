// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Retry policy and primary execution functions (`execute`, `execute_streaming`).

use std::ops::AsyncFn;
use std::time::Duration;

use super::endpoints::{Answer, Endpoint, StreamAnswer};
use super::fallback::{send, send_response};
use crate::client::{Http, HttpResponse};
use crate::error::{FetchError, ResponseFormat};

/// How many times a request is sent before giving up. One retry covers a
/// dropped connection; more would just multiply load on an endpoint that is
/// already struggling.
pub(super) const MAX_ATTEMPTS: u32 = 2;

/// How long to wait between attempts.
pub(super) const RETRY_BACKOFF: Duration = Duration::from_millis(400);

/// POST `query` to `endpoint` in `format`, retrying only what a retry can fix.
///
/// # Errors
/// Returns [`FetchError`] if the request could not be sent, if the endpoint
/// answered with a non-2xx status, or if it answered with an empty body. A
/// failure that a retry cannot fix is returned on the first attempt rather than
/// after the backoff.
pub async fn execute<H: Http>(
    http: &H,
    endpoint: Endpoint,
    query: &str,
    format: ResponseFormat,
) -> Result<Answer, FetchError> {
    let target = endpoint.url().to_string();
    let body = with_retry(http, &target, query, format, send).await?;
    Ok(Answer { endpoint, body })
}

/// POST `query`, and hand back its body to be read in chunks instead of at once.
///
/// The difference from [`execute`] is the point: a payload that does not fit in memory
/// cannot be fetched by a method that assembles it first.
///
/// # Errors
/// Returns [`FetchError`] if the request could not be sent or the endpoint
/// answered with a non-2xx status. A failure a retry cannot fix is returned on
/// the first attempt rather than after the backoff.
pub async fn execute_streaming<H: Http>(
    http: &H,
    endpoint: Endpoint,
    query: &str,
    format: ResponseFormat,
) -> Result<StreamAnswer, FetchError> {
    let target = endpoint.url().to_string();
    let response = with_retry(http, &target, query, format, send_response).await?;
    // Outside the retry on purpose: a transport that refuses to stream has not
    // failed in a way another attempt could fix, so it is reported now rather
    // than after the backoff.
    let chunks = response.into_chunks()?;
    Ok(StreamAnswer { endpoint, chunks })
}

/// Run one request through the retry policy both transports share.
pub(super) async fn with_retry<H, T, Run>(
    http: &H,
    target: &str,
    query: &str,
    format: ResponseFormat,
    run: Run,
) -> Result<T, FetchError>
where
    H: Http,
    Run: AsyncFn(&H, &str, &str, ResponseFormat) -> Result<T, FetchError>,
{
    for _ in 1..MAX_ATTEMPTS {
        match run(http, target, query, format).await {
            Err(err) if err.is_retryable() => backoff(RETRY_BACKOFF).await,
            result => return result,
        }
    }
    run(http, target, query, format).await
}

#[cfg(not(target_arch = "wasm32"))]
async fn backoff(duration: Duration) {
    tokio::time::sleep(duration).await;
}

#[cfg(target_arch = "wasm32")]
use super::wasm::backoff;

impl From<lotus_query::ExportFormat> for ResponseFormat {
    /// An export format names the shape the caller wants; the transport names
    /// the content type it has to ask the endpoint for.
    fn from(format: lotus_query::ExportFormat) -> Self {
        match format {
            lotus_query::ExportFormat::Csv => Self::Csv,
            lotus_query::ExportFormat::Json => Self::SparqlJson,
            lotus_query::ExportFormat::Rdf => Self::Turtle,
        }
    }
}
