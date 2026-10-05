// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The `reqwest`-backed [`Http`].
//!
//! On by default. Turn the `reqwest` feature off to build the rest of the crate
//! without any HTTP stack, which is how the parsing and query-building tests
//! run with no network available at all.

// As in `client`: a wasm `fetch` future is not `Send`, and the only thing using
// this module on wasm is the browser, which has one thread by definition.
#![cfg_attr(target_arch = "wasm32", allow(clippy::future_not_send))]

use super::error::TruncationReason;
use super::{BodyChunks, ChunkFuture, ChunkedBody, FetchError, Http, HttpResponse, ResponseBody};
use std::sync::OnceLock;

/// One client for the process, so that connections are pooled.
///
/// A `Copy` newtype over a `&'static`, so passing it around costs nothing.
#[derive(Clone, Copy)]
pub struct ReqwestClient(&'static reqwest::Client);

impl std::fmt::Debug for ReqwestClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReqwestClient")
    }
}

impl ReqwestClient {
    /// Build a client with the timeouts and pool this workload wants.
    ///
    /// # Errors
    /// Returns [`FetchError::Network`] if the TLS backend cannot be initialised.
    pub fn new() -> Result<Self, FetchError> {
        static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
        match CLIENT.get_or_init(build) {
            Ok(client) => Ok(Self(client)),
            Err(msg) => Err(FetchError::Network(msg.clone())),
        }
    }
}

impl Http for ReqwestClient {
    type Response = reqwest::Response;

    async fn post(
        &self,
        endpoint: &str,
        accept: &str,
        body: String,
    ) -> Result<reqwest::Response, FetchError> {
        self.0
            .post(endpoint)
            .header("Accept", accept)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(|e| FetchError::Network(e.to_string()))
    }

    async fn post_form(
        &self,
        endpoint: &str,
        accept: &str,
        body: String,
        headers: &[(&str, String)],
    ) -> Result<reqwest::Response, FetchError> {
        let mut request = self
            .0
            .post(endpoint)
            .header("Accept", accept)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body);
        // `api-user-agent` is how `QLever` is told who is calling, and
        // `api-token` is how it is told the caller has been given more than the
        // anonymous budget. Both are ordinary headers here; the reason they are
        // in the transport rather than in the query builder is that a header is
        // a property of the request, not of the query.
        for (name, value) in headers {
            request = request.header(*name, value);
        }
        request
            .send()
            .await
            .map_err(|e| FetchError::Network(e.to_string()))
    }

    async fn post_json(&self, url: &str, body: String) -> Result<reqwest::Response, FetchError> {
        self.0
            .post(url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| FetchError::Network(e.to_string()))
    }

    async fn get(&self, url: &str, accept: &str) -> Result<reqwest::Response, FetchError> {
        self.0
            .get(url)
            .header("Accept", accept)
            .send()
            .await
            .map_err(|e| FetchError::Network(e.to_string()))
    }
}

impl HttpResponse for reqwest::Response {
    fn status(&self) -> u16 {
        self.status().as_u16()
    }

    async fn bytes(self) -> Result<ResponseBody, FetchError> {
        self.bytes()
            .await
            .map_err(|e| FetchError::Network(e.to_string()))
    }

    async fn text(self) -> Result<String, FetchError> {
        self.text()
            .await
            .map_err(|e| FetchError::Network(e.to_string()))
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn chunk(&mut self) -> Result<Option<ResponseBody>, FetchError> {
        // The inherent method wins here: this is `reqwest::Response::chunk`.
        self.chunk()
            .await
            .map_err(|e| FetchError::Network(e.to_string()))
    }

    // The wasm `Response` has no inherent `chunk`, so this exists to satisfy the
    // trait and is never called: the streaming path goes through `into_chunks`.
    //
    // `HttpResponse` is an async-fn-in-trait, so `async fn` is the spelling the
    // trait requires; there is nothing to await because nothing runs.
    #[cfg(target_arch = "wasm32")]
    #[expect(
        clippy::unused_async_trait_impl,
        reason = "HttpResponse is an AFIT, and this wasm stub is never called"
    )]
    async fn chunk(&mut self) -> Result<Option<ResponseBody>, FetchError> {
        Err(FetchError::Network(
            "incremental reads go through `into_chunks` here".into(),
        ))
    }

    fn into_chunks(self) -> Result<ChunkedBody, FetchError> {
        #[cfg(target_arch = "wasm32")]
        {
            // `bytes_stream` is the browser's `ReadableStream` underneath, so the
            // body arrives already decompressed and in pieces. That is exactly
            // what is wanted: the decompressed payload is never in one piece.
            Ok(Box::new(WasmChunks {
                inner: Box::pin(self.bytes_stream()),
                bytes_read: 0,
            }))
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            Ok(Box::new(NativeChunks {
                response: self,
                bytes_read: 0,
            }))
        }
    }
}

/// How long a streamed body may go without yielding a byte before it is called
/// stalled.
///
/// An idle deadline, not an overall one. A wide search is minutes of legitimate transfer, so
/// a wall-clock cap would reject the very results it is meant to bound; but a body that stops
/// moving is one whose reader has already returned, silently, with whatever arrived. Without
/// this, "still working" and "gave up" are indistinguishable.
///
/// Generous enough not to fire on a slow query still computing its first chunk: `QLever` can
/// think for a while before the first byte of a wide answer.
pub const STREAM_STALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);

/// Why a transport error is not being reported as one.
///
/// A read that fails before a single byte arrived is a connection problem, and
/// repeating the request is free. The same error after a megabyte has arrived is
/// a body that died mid-transfer, and repeating the request re-sends all of it
/// to reach the same point. The caller can only act differently if the two are
/// distinguishable, which is the whole reason this is a function.
fn classify_read_failure(bytes_read: u64, message: String) -> FetchError {
    if bytes_read > 0 {
        FetchError::Truncated {
            bytes_read,
            reason: TruncationReason::ClosedEarly,
        }
    } else {
        FetchError::Network(message)
    }
}

/// Native streaming: the response's own `chunk`, which reads as the socket
/// delivers and holds no more than one chunk.
#[cfg(not(target_arch = "wasm32"))]
struct NativeChunks {
    response: reqwest::Response,
    bytes_read: u64,
}

#[cfg(not(target_arch = "wasm32"))]
impl BodyChunks for NativeChunks {
    fn next_chunk(&mut self) -> ChunkFuture<'_> {
        Box::pin(async {
            let chunk = tokio::time::timeout(STREAM_STALL_TIMEOUT, self.response.chunk())
                .await
                .map_err(|_| FetchError::Truncated {
                    bytes_read: self.bytes_read,
                    reason: TruncationReason::Stalled,
                })?
                .map_err(|e| classify_read_failure(self.bytes_read, e.to_string()))?;
            if let Some(bytes) = &chunk {
                self.bytes_read += bytes.len() as u64;
            }
            Ok(chunk)
        })
    }
}

/// Wasm streaming: `reqwest`'s `ReadableStream` as a `Stream` of `Bytes`.
#[cfg(target_arch = "wasm32")]
struct WasmChunks {
    inner: std::pin::Pin<Box<dyn futures::Stream<Item = Result<ResponseBody, reqwest::Error>>>>,
    bytes_read: u64,
}

#[cfg(target_arch = "wasm32")]
impl BodyChunks for WasmChunks {
    fn next_chunk(&mut self) -> ChunkFuture<'_> {
        Box::pin(async {
            use futures::StreamExt as _;
            // Read the counter before `next` borrows the stream, so the error
            // can say how far the body got.
            let bytes_read = self.bytes_read;
            let next = self.inner.next();
            // `gloo-timers` counts whole milliseconds in a `u32`, where `tokio`
            // takes a `Duration`. Saturating rather than `as`, so a value past
            // the ceiling would wrap to a near-instant timeout rather than a
            // near-infinite one.
            let millis = u32::try_from(STREAM_STALL_TIMEOUT.as_millis()).unwrap_or(u32::MAX);
            let idle = gloo_timers::future::TimeoutFuture::new(millis);
            futures::pin_mut!(next);
            // `select` is biased to its first argument, so a chunk that lands in
            // the same tick as the deadline wins. That ordering is the whole
            // point: the deadline must not eat data that has already arrived.
            match futures::future::select(next, idle).await {
                futures::future::Either::Left((chunk, _idle)) => {
                    let chunk = chunk
                        .transpose()
                        .map_err(|e| classify_read_failure(bytes_read, e.to_string()))?;
                    if let Some(bytes) = &chunk {
                        self.bytes_read += bytes.len() as u64;
                    }
                    Ok(chunk)
                }
                futures::future::Either::Right((_elapsed, _next)) => Err(FetchError::Truncated {
                    bytes_read,
                    reason: TruncationReason::Stalled,
                }),
            }
        })
    }
}

fn build() -> Result<reqwest::Client, String> {
    #[cfg(target_arch = "wasm32")]
    {
        // The browser sends `Accept-Encoding` and decodes gzip itself, and
        // manages the connection pool; there is nothing to configure.
        reqwest::Client::builder()
            .build()
            .map_err(|e| e.to_string())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::Duration;
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_mins(2))
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(32)
            .tcp_keepalive(Duration::from_secs(30))
            .gzip(true)
            .build()
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_read_that_failed_before_any_byte_is_still_a_connection_problem() {
        // Nothing was transferred, so sending the request again costs nothing
        // and might work. This is the retryable half.
        let err = classify_read_failure(0, "connection reset".into());
        assert!(matches!(err, FetchError::Network(_)), "{err:?}");
        assert!(err.is_retryable());
    }

    #[test]
    fn a_read_that_failed_after_bytes_is_a_cut_short_result_set() {
        // This is the rule the timeout complaint actually turns on: the same
        // socket error, but with half a gigabyte already transferred, and so
        // not something to answer by starting over.
        let err = classify_read_failure(4096, "connection reset".into());
        assert!(err.is_truncated(), "{err:?}");
        assert!(!err.is_retryable());
    }

    #[test]
    fn the_one_byte_threshold_is_a_real_boundary_and_not_a_range() {
        // Mutating `> 0` to `>= 0` would make every failure untouchable if it
        // were wrong in the other direction, and `>= 1` to `> 0` would make every
        // failure retryable. Both are single-character changes with opposite
        // effects, so both ends are pinned.
        assert!(classify_read_failure(0, "x".into()).is_retryable());
        assert!(classify_read_failure(1, "x".into()).is_truncated());
    }
}
