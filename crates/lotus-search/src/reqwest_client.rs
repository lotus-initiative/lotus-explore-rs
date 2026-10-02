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
            }))
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            Ok(Box::new(NativeChunks { response: self }))
        }
    }
}

/// Native streaming: the response's own `chunk`, which reads as the socket
/// delivers and holds no more than one chunk.
#[cfg(not(target_arch = "wasm32"))]
struct NativeChunks {
    response: reqwest::Response,
}

#[cfg(not(target_arch = "wasm32"))]
impl BodyChunks for NativeChunks {
    fn next_chunk(&mut self) -> ChunkFuture<'_> {
        Box::pin(async {
            self.response
                .chunk()
                .await
                .map_err(|e| FetchError::Network(e.to_string()))
        })
    }
}

/// Wasm streaming: `reqwest`'s `ReadableStream` as a `Stream` of `Bytes`.
#[cfg(target_arch = "wasm32")]
struct WasmChunks {
    inner: std::pin::Pin<Box<dyn futures::Stream<Item = Result<ResponseBody, reqwest::Error>>>>,
}

#[cfg(target_arch = "wasm32")]
impl BodyChunks for WasmChunks {
    fn next_chunk(&mut self) -> ChunkFuture<'_> {
        Box::pin(async {
            use futures::StreamExt;
            self.inner
                .next()
                .await
                .transpose()
                .map_err(|e| FetchError::Network(e.to_string()))
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
