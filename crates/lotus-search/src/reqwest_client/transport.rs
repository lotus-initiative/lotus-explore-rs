// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Native `reqwest` socket timeouts, client builder, and chunked body reader.

use super::ClientBuildError;
#[cfg(not(target_arch = "wasm32"))]
use super::{BodyChunks, ChunkFuture, FetchError, ReadProgress, ResponseBody};

/// How long a streamed body may go without yielding a byte before it is called stalled.
pub const STREAM_STALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);

/// How long the client lets a connection go without a byte, headers included.
pub(super) const CONNECTION_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);

/// Native streaming: the response's own `chunk`, which reads as the socket
/// delivers and holds no more than one chunk.
#[cfg(not(target_arch = "wasm32"))]
pub(super) struct NativeChunks {
    pub(super) response: reqwest::Response,
    pub(super) progress: ReadProgress,
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
pub(super) async fn send_request(
    request: reqwest::RequestBuilder,
) -> Result<reqwest::Response, FetchError> {
    request
        .send()
        .await
        .map_err(|e| FetchError::Network(e.to_string()))
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) async fn read_bytes(response: reqwest::Response) -> Result<ResponseBody, FetchError> {
    response
        .bytes()
        .await
        .map_err(|e| FetchError::Network(e.to_string()))
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) async fn read_text(response: reqwest::Response) -> Result<String, FetchError> {
    response
        .text()
        .await
        .map_err(|e| FetchError::Network(e.to_string()))
}

pub(super) fn build() -> Result<reqwest::Client, ClientBuildError> {
    #[cfg(target_arch = "wasm32")]
    {
        Ok(reqwest::Client::builder().build()?)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        Ok(native_builder(CONNECTION_IDLE_TIMEOUT).build()?)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn native_builder(idle: std::time::Duration) -> reqwest::ClientBuilder {
    use std::time::Duration;
    reqwest::Client::builder()
        .user_agent(crate::CLIENT_ID)
        .connect_timeout(Duration::from_secs(8))
        .read_timeout(idle)
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(32)
        .tcp_keepalive(Duration::from_secs(30))
        .gzip(true)
}
