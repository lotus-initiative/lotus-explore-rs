// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The `reqwest`-backed [`Http`].
//!
//! On by default. Turn the `reqwest` feature off to build the rest of the crate
//! without any HTTP stack, which is how the parsing and query-building tests
//! run with no network available at all.

#![cfg_attr(
    target_arch = "wasm32",
    allow(
        clippy::future_not_send,
        reason = "browser fetch futures are single-threaded and !Send"
    )
)]

mod schedule;
mod transport;
#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(test)]
mod tests;

use std::sync::OnceLock;

use super::error::TruncationReason;
use super::{BodyChunks, ChunkFuture, ChunkedBody, FetchError, Http, HttpResponse, ResponseBody};
use transport::build;
#[cfg(not(target_arch = "wasm32"))]
use transport::{NativeChunks, read_bytes, read_text, send_request};

#[cfg(target_arch = "wasm32")]
use wasm::{read_bytes, read_text, send_request};
#[cfg(target_arch = "wasm32")]
pub use wasm::{sleep_wall_clock, wait_deadline, yield_to_browser};

#[cfg(any(target_arch = "wasm32", test))]
use schedule::{DeadlineTracker, RESUME_GRACE_MS, YieldSchedule, duration_millis_u32};

#[cfg(any(target_arch = "wasm32", test))]
use transport::CONNECTION_IDLE_TIMEOUT;
pub use transport::STREAM_STALL_TIMEOUT;
#[cfg(all(test, not(target_arch = "wasm32")))]
use transport::native_builder;

/// Typed client-initialisation error cached in the shared [`ReqwestClient`] lock.
#[derive(Debug, thiserror::Error)]
#[error("failed to initialise HTTP client: {0}")]
struct ClientBuildError(#[from] reqwest::Error);

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
        static CLIENT: OnceLock<Result<reqwest::Client, ClientBuildError>> = OnceLock::new();
        match CLIENT.get_or_init(build) {
            Ok(client) => Ok(Self(client)),
            Err(err) => Err(FetchError::Network(err.to_string())),
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
        self.chunk()
            .await
            .map_err(|e| FetchError::Network(e.to_string()))
    }

    fn into_chunks(self) -> Result<ChunkedBody, FetchError> {
        #[cfg(target_arch = "wasm32")]
        {
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

/// How far a streamed body has got, and what a failure at that point means.
#[derive(Debug, Default)]
pub(super) struct ReadProgress {
    bytes_read: u64,
}

impl ReadProgress {
    /// Count a chunk that arrived, and hand it on.
    pub(super) const fn arrived(&mut self, chunk: Option<ResponseBody>) -> Option<ResponseBody> {
        if let Some(bytes) = &chunk {
            self.bytes_read += bytes.len() as u64;
        }
        chunk
    }

    /// A body that went quiet for longer than [`STREAM_STALL_TIMEOUT`].
    pub(super) const fn stalled(&self) -> FetchError {
        FetchError::Truncated {
            bytes_read: self.bytes_read,
            reason: TruncationReason::Stalled,
        }
    }

    /// Why a transport error is not being reported as one.
    pub(super) fn failed(&self, message: String) -> FetchError {
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
