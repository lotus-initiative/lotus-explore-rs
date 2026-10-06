// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
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
