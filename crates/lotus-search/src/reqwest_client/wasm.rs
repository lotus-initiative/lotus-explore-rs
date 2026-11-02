// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The browser's streaming reader. Only compiled for wasm32, so no native test
//! can observe it; `.cargo/mutants.toml` excludes this file for that reason.

use super::{BodyChunks, ChunkFuture, ReadProgress, ResponseBody, STREAM_STALL_TIMEOUT};

/// Wasm streaming: `reqwest`'s `ReadableStream` as a `Stream` of `Bytes`.
pub(super) struct WasmChunks {
    pub(super) inner:
        std::pin::Pin<Box<dyn futures::Stream<Item = Result<ResponseBody, reqwest::Error>>>>,
    pub(super) progress: ReadProgress,
}

impl BodyChunks for WasmChunks {
    fn next_chunk(&mut self) -> ChunkFuture<'_> {
        Box::pin(async {
            use futures::StreamExt as _;
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
                        .map_err(|e| self.progress.failed(e.to_string()))?;
                    Ok(self.progress.arrived(chunk))
                }
                futures::future::Either::Right((_elapsed, _next)) => Err(self.progress.stalled()),
            }
        })
    }
}
