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
/// What makes a result set larger than memory possible: the payload is never assembled, so
/// peak cost is the finished set plus one chunk. [`HttpResponse::text`] costs the whole
/// payload at once -- for the widest search measured, 2,990,730 edges at the 314 B/row a
/// real export averages, about 940 MB of CSV against a 200 MB budget.
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
    /// Separate from [`Http::post`] because only some transports can set headers and the
    /// others should not have to pretend: the default ignores them, correct for a scripted
    /// test double, which is why this is an added method rather than a changed signature.
    /// Every implementation here that talks to a real service overrides it.
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

#[cfg(test)]
mod tests {
    //! The default methods on the two traits, which are what a transport gets for
    //! free when it does not need the capability.
    //!
    //! `post_json` and `into_chunks` are refusals, not fallbacks, and both were untested: every
    //! transport in this crate's suite is scripted and overrides what it needs, so the
    //! defaults were compiled and never called. That left the one behaviour a
    //! *third-party* implementor depends on -- the error they get when they do not
    //! override -- unpinned.
    //!
    //! The stubs below implement the four required methods and override nothing
    //! else, which is exactly the case the defaults exist for.

    // A test that fails on an unexpected error shape is reporting, not panicking,
    // and the `match` that reports it cannot be written without `panic!`. The
    // same reasoning the integration tests in `tests/` use.
    #![allow(clippy::panic)]

    use super::*;
    use std::future::ready;

    /// A response that cannot be streamed, because it does not override
    /// `into_chunks`.
    #[derive(Debug, Clone)]
    struct Opaque {
        body: Vec<u8>,
    }

    impl HttpResponse for Opaque {
        fn status(&self) -> u16 {
            200
        }

        fn bytes(self) -> impl Future<Output = Result<ResponseBody, FetchError>> {
            ready(Ok(ResponseBody::from(self.body)))
        }

        fn text(self) -> impl Future<Output = Result<String, FetchError>> {
            ready(Ok(String::from_utf8_lossy(&self.body).into_owned()))
        }

        fn chunk(&mut self) -> impl Future<Output = Result<Option<ResponseBody>, FetchError>> {
            ready(Ok(if self.body.is_empty() {
                None
            } else {
                Some(ResponseBody::from(std::mem::take(&mut self.body)))
            }))
        }
    }

    /// A transport that speaks neither JSON nor chunked bodies.
    #[derive(Debug, Clone)]
    struct Minimal;

    impl Http for Minimal {
        type Response = Opaque;

        fn post(
            &self,
            _endpoint: &str,
            _accept: &str,
            _body: String,
        ) -> impl Future<Output = Result<Self::Response, FetchError>> {
            ready(Ok(Opaque {
                body: b"posted".to_vec(),
            }))
        }

        fn get(
            &self,
            _url: &str,
            _accept: &str,
        ) -> impl Future<Output = Result<Self::Response, FetchError>> {
            ready(Ok(Opaque {
                body: b"got".to_vec(),
            }))
        }
    }

    /// A transport that cannot read a body incrementally says so, and names the
    /// capability that is missing.
    ///
    /// The message is what a caller reads to tell "this transport cannot do that"
    /// apart from a network failure, which is the entire reason this is a
    /// distinct error rather than an empty `Ok`. A caller needing a large result
    /// to fit has to be able to act on it.
    #[tokio::test]
    async fn a_transport_that_cannot_stream_refuses_and_names_what_is_missing() {
        let refusal = Opaque { body: Vec::new() }.into_chunks();

        let message = match refusal {
            Ok(_) => panic!("a body that cannot be read in chunks must not report success"),
            Err(FetchError::Network(message)) => message,
            Err(other) => panic!("expected a network refusal, got {other:?}"),
        };
        assert!(
            message.contains("in chunks"),
            "the message must name the missing capability, got {message:?}"
        );
    }

    /// A transport that cannot POST JSON refuses rather than sending a form.
    ///
    /// Converting a structure to an `InChIKey` is a JSON POST, and there is no form
    /// encoding that expresses it. The default must not fall back to `post`: that
    /// would send a form encoding to an endpoint expecting JSON, and the failure
    /// would surface as a parse error a long way from the cause.
    #[tokio::test]
    async fn a_transport_that_cannot_post_json_refuses_rather_than_guessing() {
        let refusal = Minimal
            .post_json("https://example.inchikey", "{}".to_string())
            .await;

        let message = match refusal {
            Ok(_) => panic!("a transport that cannot POST JSON must not report success"),
            Err(FetchError::Network(message)) => message,
            Err(other) => panic!("expected a network refusal, got {other:?}"),
        };
        assert!(
            message.contains("JSON"),
            "the message must name the missing capability, got {message:?}"
        );
    }

    /// `post_form` defaults to `post`, which means the headers are dropped.
    ///
    /// That is correct for a scripted double and is the stated reason this is an
    /// added method rather than a changed signature, so it is worth pinning: a
    /// default that started honouring headers would change what every implementor
    /// that does not expect to be asked gets.
    #[tokio::test]
    async fn the_default_post_form_ignores_the_headers_it_is_given() {
        let sent = [("api-token", "secret".to_string())];

        let response = Minimal
            .post_form("https://example.test", "text/csv", "q=1".into(), &sent)
            .await;

        match response {
            Ok(response) => assert_eq!(response.status(), 200),
            Err(error) => panic!("the default delegates to post, and post succeeds: {error:?}"),
        }
    }
}
