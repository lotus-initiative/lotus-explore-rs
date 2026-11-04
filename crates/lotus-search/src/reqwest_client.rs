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
        // `Authorization: Bearer <token>` is how it is told the caller has been
        // given more than the anonymous budget. Both are ordinary headers here;
        // the reason they are in the transport rather than in the query builder
        // is that a header is a property of the request, not of the query.
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

    fn into_chunks(self) -> Result<ChunkedBody, FetchError> {
        #[cfg(target_arch = "wasm32")]
        {
            // `bytes_stream` is the browser's `ReadableStream` underneath, so the
            // body arrives already decompressed and in pieces. That is exactly
            // what is wanted: the decompressed payload is never in one piece.
            Ok(Box::new(wasm::WasmChunks {
                inner: Box::pin(self.bytes_stream()),
                progress: ReadProgress::default(),
                bytes_since_yield: 0,
            }))
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            Ok(Box::new(NativeChunks {
                response: self,
                progress: ReadProgress::default(),
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

/// How far a streamed body has got, and what a failure at that point means.
///
/// The one piece of logic both platforms' readers share. It lives here rather
/// than in each reader because the wasm reader is not compiled into a native
/// test binary: kept there, a mistake in the count could only be found in a
/// browser, while here the native suite reaches it.
#[derive(Debug, Default)]
struct ReadProgress {
    bytes_read: u64,
}

impl ReadProgress {
    /// Count a chunk that arrived, and hand it on.
    const fn arrived(&mut self, chunk: Option<ResponseBody>) -> Option<ResponseBody> {
        if let Some(bytes) = &chunk {
            self.bytes_read += bytes.len() as u64;
        }
        chunk
    }

    /// A body that went quiet for longer than [`STREAM_STALL_TIMEOUT`].
    const fn stalled(&self) -> FetchError {
        FetchError::Truncated {
            bytes_read: self.bytes_read,
            reason: TruncationReason::Stalled,
        }
    }

    /// Why a transport error is not being reported as one.
    ///
    /// A read that fails before a single byte arrived is a connection problem, and
    /// repeating the request is free. The same error after a megabyte has arrived is
    /// a body that died mid-transfer, and repeating the request re-sends all of it
    /// to reach the same point. The caller can only act differently if the two are
    /// distinguishable, which is the whole reason this is a function.
    fn failed(&self, message: String) -> FetchError {
        if self.bytes_read > 0 {
            FetchError::Truncated {
                bytes_read: self.bytes_read,
                reason: TruncationReason::ClosedEarly,
            }
        } else {
            FetchError::Network(message)
        }
    }
}

/// Native streaming: the response's own `chunk`, which reads as the socket
/// delivers and holds no more than one chunk.
#[cfg(not(target_arch = "wasm32"))]
struct NativeChunks {
    response: reqwest::Response,
    progress: ReadProgress,
}

#[cfg(not(target_arch = "wasm32"))]
impl BodyChunks for NativeChunks {
    fn next_chunk(&mut self) -> ChunkFuture<'_> {
        Box::pin(async {
            let chunk = tokio::time::timeout(STREAM_STALL_TIMEOUT, self.response.chunk())
                .await
                .map_err(|_| self.progress.stalled())?
                .map_err(|e| self.progress.failed(e.to_string()))?;
            Ok(self.progress.arrived(chunk))
        })
    }
}

// The browser's half lives in its own file because it is never compiled into a
// native test binary, so `.cargo/mutants.toml` can exclude it by path instead of
// by line number.
#[cfg(target_arch = "wasm32")]
#[path = "reqwest_client/wasm.rs"]
mod wasm;

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
#[path = "reqwest_client/tests.rs"]
mod tests;
