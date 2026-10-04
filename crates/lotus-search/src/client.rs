// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The transport seam: two methods, so that everything above can be tested
//! without a network.

// A `fetch` future is not `Send`: the browser's is single-threaded and has no
// reactor to hand work to another thread. The allow is on the wasm build only,
// because on native a `Send` future is a real requirement and worth keeping.
#![cfg_attr(target_arch = "wasm32", allow(clippy::future_not_send))]

use crate::error::FetchError;
use std::future::Future;
use std::pin::Pin;

/// A response body, not yet decoded.
pub type ResponseBody = bytes::Bytes;

/// What [`BodyChunks::next_chunk`] hands back.
///
/// A boxed future rather than an `async fn` in the trait, because a trait whose
/// methods return `impl Future` cannot have a `Box<dyn Trait>` of it, and this
/// one has to be held behind a box: the two platforms get their chunks from two
/// unrelated pieces of their HTTP stacks.
///
/// The lifetime is tied to the borrow of the reader, not `'static`, because the
/// future reads from that reader. It is `Send` on native and not on wasm, because
/// the browser's stream is not `Send` and there is only one thread to send it to.
#[cfg(not(target_arch = "wasm32"))]
pub type ChunkFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<ResponseBody>, FetchError>> + Send + 'a>>;

/// What [`BodyChunks::next_chunk`] hands back, in the browser.
#[cfg(target_arch = "wasm32")]
pub type ChunkFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<ResponseBody>, FetchError>> + 'a>>;

/// A response body being read a chunk at a time.
///
/// This is what makes a result set larger than memory possible: the payload is
/// never assembled, so peak cost is the finished set plus one chunk. Reading a
/// response with [`HttpResponse::text`] instead costs the whole payload at once,
/// which for the widest search measured is 2,990,730 edges at the 314 B/row a real export measures is about 940 MB of CSV against a 200 MB
/// budget.
pub trait BodyChunks {
    /// The next chunk, or `None` at end of body.
    fn next_chunk(&mut self) -> ChunkFuture<'_>;
}

/// A response body held for incremental reading.
///
/// `Send` on native, not on wasm: the browser's stream is not `Send`, and the
/// module-level allow for `future_not_send` is about exactly that. Nothing
/// needs it to be, because there is one thread.
#[cfg(not(target_arch = "wasm32"))]
pub type ChunkedBody = Box<dyn BodyChunks + Send>;

/// A response body held for incremental reading, in the browser.
#[cfg(target_arch = "wasm32")]
pub type ChunkedBody = Box<dyn BodyChunks>;

/// A response being read.
///
/// `bytes` and `text` consume the response because they read to the end;
/// `chunk` borrows it so that a large result can be streamed to a file instead
/// of being held in memory.
pub trait HttpResponse {
    /// The status code, available as soon as the headers arrive.
    fn status(&self) -> u16;

    /// Read the whole body as bytes.
    fn bytes(self) -> impl Future<Output = Result<ResponseBody, FetchError>>;

    /// Read the whole body as UTF-8.
    fn text(self) -> impl Future<Output = Result<String, FetchError>>;

    /// Read the next chunk of the body, or `None` at end of input.
    fn chunk(&mut self) -> impl Future<Output = Result<Option<ResponseBody>, FetchError>>;

    /// Take the response apart so its body can be read in chunks.
    ///
    /// The default refuses, because a transport that cannot stream still has to
    /// satisfy the trait. Callers that need a large body to fit should treat the
    /// refusal as "this transport cannot do that" rather than as a network
    /// error, which is why it is a distinct message.
    ///
    /// # Errors
    /// Returns [`FetchError::Network`] if this transport has no way to read a
    /// body incrementally, which the default implementation always does.
    fn into_chunks(self) -> Result<ChunkedBody, FetchError>
    where
        Self: Sized,
    {
        Err(FetchError::Network(
            "this transport cannot read a response body in chunks".into(),
        ))
    }
}

/// Sends SPARQL queries.
///
/// Implement this to run the workspace against a recorded fixture, a proxy, or
/// an HTTP stack the platform already has.
pub trait Http: Clone + Send + Sync + 'static {
    /// The response type this client produces.
    type Response: HttpResponse;

    /// POST a form-encoded query, asking for `accept`.
    fn post(
        &self,
        endpoint: &str,
        accept: &str,
        body: String,
    ) -> impl Future<Output = Result<Self::Response, FetchError>>;

    /// GET an arbitrary URL, asking for `accept`.
    fn get(
        &self,
        url: &str,
        accept: &str,
    ) -> impl Future<Output = Result<Self::Response, FetchError>>;

    /// POST a form-encoded query **with request headers**, asking for `accept`.
    ///
    /// Separate from [`Http::post`] because only some transports can set headers,
    /// and because the ones that cannot should not have to pretend: the default
    /// ignores them, which is correct for a scripted test double and is the whole
    /// reason this is an added method rather than a changed signature. Every
    /// implementation in this workspace that talks to a real service overrides
    /// it.
    fn post_form(
        &self,
        endpoint: &str,
        accept: &str,
        body: String,
        _headers: &[(&str, String)],
    ) -> impl Future<Output = Result<Self::Response, FetchError>> {
        self.post(endpoint, accept, body)
    }

    /// POST a JSON body, for the APIs that take one.
    ///
    /// Not every service curation touches speaks SPARQL: converting a structure
    /// to an `InChIKey` is a JSON POST, and there is no form encoding that
    /// expresses it. The default refuses rather than guessing, so a transport
    /// that only speaks SPARQL still compiles and the caller is told what is
    /// missing instead of sending something malformed.
    fn post_json(
        &self,
        _url: &str,
        _body: String,
    ) -> impl Future<Output = Result<Self::Response, FetchError>> {
        std::future::ready(Err(FetchError::Network(
            "this transport cannot POST a JSON body".into(),
        )))
    }
}
