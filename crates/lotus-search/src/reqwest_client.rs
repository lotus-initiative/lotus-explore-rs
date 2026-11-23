// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The `reqwest`-backed [`Http`].
//!
//! On by default. Turn the `reqwest` feature off to build the rest of the crate
//! without any HTTP stack, which is how the parsing and query-building tests
//! run with no network available at all.

// As in `client`: a wasm `fetch` future is not `Send`, and the only thing using
// this module on wasm is the browser, which has one thread by definition.
#![cfg_attr(
    target_arch = "wasm32",
    allow(
        clippy::future_not_send,
        reason = "browser fetch futures are single-threaded and !Send"
    )
)]

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
        let request = self
            .0
            .post(endpoint)
            .header("Accept", accept)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body);
        send_request(request).await
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
        send_request(request).await
    }

    async fn post_json(&self, url: &str, body: String) -> Result<reqwest::Response, FetchError> {
        let request = self
            .0
            .post(url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .body(body);
        send_request(request).await
    }

    async fn get(&self, url: &str, accept: &str) -> Result<reqwest::Response, FetchError> {
        let request = self.0.get(url).header("Accept", accept);
        send_request(request).await
    }
}

impl HttpResponse for reqwest::Response {
    fn status(&self) -> u16 {
        self.status().as_u16()
    }

    async fn bytes(self) -> Result<ResponseBody, FetchError> {
        read_bytes(self).await
    }

    async fn text(self) -> Result<String, FetchError> {
        read_text(self).await
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
            Ok(Box::new(wasm::WasmChunks::new(self)))
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

#[cfg(not(target_arch = "wasm32"))]
async fn send_request(request: reqwest::RequestBuilder) -> Result<reqwest::Response, FetchError> {
    request
        .send()
        .await
        .map_err(|e| FetchError::Network(e.to_string()))
}

#[cfg(not(target_arch = "wasm32"))]
async fn read_bytes(response: reqwest::Response) -> Result<ResponseBody, FetchError> {
    response
        .bytes()
        .await
        .map_err(|e| FetchError::Network(e.to_string()))
}

#[cfg(not(target_arch = "wasm32"))]
async fn read_text(response: reqwest::Response) -> Result<String, FetchError> {
    response
        .text()
        .await
        .map_err(|e| FetchError::Network(e.to_string()))
}

// The browser's half lives in its own file because it is never compiled into a
// native test binary, so `.cargo/mutants.toml` can exclude it by path instead of
// by line number.
#[cfg(target_arch = "wasm32")]
#[path = "reqwest_client/wasm.rs"]
mod wasm;

#[cfg(target_arch = "wasm32")]
use wasm::{read_bytes, read_text, send_request};

#[cfg(target_arch = "wasm32")]
pub use wasm::{sleep_wall_clock, wait_deadline, yield_to_browser};

#[cfg(any(target_arch = "wasm32", test))]
const VISIBLE_YIELD_INTERVAL_MS: u64 = 16;

#[cfg(any(target_arch = "wasm32", test))]
const VISIBLE_YIELD_BYTES: usize = 262_144;

#[cfg(any(target_arch = "wasm32", test))]
const HIDDEN_YIELD_INTERVAL_MS: u64 = 100;

#[cfg(any(target_arch = "wasm32", test))]
const HIDDEN_YIELD_BYTES: usize = 1_048_576;

#[cfg(any(target_arch = "wasm32", test))]
const RESUME_GRACE_MS: u32 = 2_000;

/// Cadence tracker deciding when browser chunk reading should yield a macrotask
/// so paint/input stay responsive when visible without throttling to timer rates
/// when hidden.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct YieldSchedule {
    last_yield_ms: u64,
    bytes_since_yield: usize,
}

#[cfg(any(target_arch = "wasm32", test))]
impl YieldSchedule {
    const fn new(now_ms: u64) -> Self {
        Self {
            last_yield_ms: now_ms,
            bytes_since_yield: 0,
        }
    }

    const fn record_bytes(&mut self, bytes: usize) {
        self.bytes_since_yield = self.bytes_since_yield.saturating_add(bytes);
    }

    const fn should_yield(&self, now_ms: u64, hidden: bool) -> bool {
        if self.bytes_since_yield == 0 {
            return false;
        }
        let (interval_ms, byte_cap) = if hidden {
            (HIDDEN_YIELD_INTERVAL_MS, HIDDEN_YIELD_BYTES)
        } else {
            (VISIBLE_YIELD_INTERVAL_MS, VISIBLE_YIELD_BYTES)
        };
        now_ms.saturating_sub(self.last_yield_ms) >= interval_ms
            || self.bytes_since_yield >= byte_cap
    }

    const fn mark_yielded(&mut self, now_ms: u64) {
        self.bytes_since_yield = 0;
        self.last_yield_ms = now_ms;
    }
}

/// Convert a [`Duration`](std::time::Duration) into whole milliseconds,
/// saturating at `u32::MAX` for browser `setTimeout`.
#[cfg(any(target_arch = "wasm32", test))]
fn duration_millis_u32(duration: std::time::Duration) -> u32 {
    u32::try_from(duration.as_millis()).unwrap_or(u32::MAX)
}

/// Wall-clock deadline tracker across browser background throttling or sleep,
/// with an optional one-shot foreground grace window on resume.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DeadlineTracker {
    deadline_ms: u64,
    resume_grace_ms: u32,
    grace_armed: bool,
}

#[cfg(any(target_arch = "wasm32", test))]
impl DeadlineTracker {
    fn new(now_ms: u64, duration: std::time::Duration, resume_grace_ms: u32) -> Self {
        Self {
            deadline_ms: now_ms.saturating_add(u64::from(duration_millis_u32(duration))),
            resume_grace_ms,
            grace_armed: false,
        }
    }

    fn remaining_on_resume(&mut self, now_ms: u64) -> u32 {
        let remaining = self.deadline_ms.saturating_sub(now_ms);
        if remaining > 0 {
            return u32::try_from(remaining).unwrap_or(u32::MAX);
        }
        if self.grace_armed || self.resume_grace_ms == 0 {
            return 0;
        }
        self.grace_armed = true;
        self.deadline_ms = now_ms.saturating_add(u64::from(self.resume_grace_ms));
        self.resume_grace_ms
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
        native_builder(CONNECTION_IDLE_TIMEOUT)
            .build()
            .map_err(|e| e.to_string())
    }
}

/// How long the client lets a connection go without a byte, headers
/// included.
///
/// Longer than any endpoint's own time limit -- WDQS computes for up to 60 s
/// before its first byte -- so a slow but legitimate answer is not cut off.
/// A streamed body has its own, shorter watchdog in [`STREAM_STALL_TIMEOUT`];
/// this is the backstop for everything else.
pub(crate) const CONNECTION_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);

/// The native client, bounded by how long the connection may sit idle.
///
/// An idle deadline and no overall one, for the reason [`STREAM_STALL_TIMEOUT`]
/// gives: reqwest's `timeout` covers the whole request, body included, so the
/// two-minute cap it used to carry failed every streamed export that took
/// longer, however steadily it was arriving -- and reported it as the
/// connection closing early. The endpoint's own time budget bounds how long it
/// computes; this bounds how long it may go quiet.
#[cfg(not(target_arch = "wasm32"))]
fn native_builder(idle: std::time::Duration) -> reqwest::ClientBuilder {
    use std::time::Duration;
    reqwest::Client::builder()
        // Wikimedia's REST Gateway rejects native requests without a `User-Agent`
        // header with HTTP 403 (`T400119`); wasm builds inherit the browser's header.
        .user_agent(crate::CLIENT_ID)
        .connect_timeout(Duration::from_secs(8))
        .read_timeout(idle)
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(32)
        .tcp_keepalive(Duration::from_secs(30))
        .gzip(true)
}

#[cfg(test)]
#[path = "reqwest_client/tests.rs"]
mod tests;
