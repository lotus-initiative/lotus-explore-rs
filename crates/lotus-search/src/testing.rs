// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! A transport that answers from a script, for tests.
//!
//! A test states the whole conversation it expects and gets it back, which matters more the
//! further a query is from the user: a curation run issues three or four requests per row
//! against a service whose data moves, so a test talking to the real service fails when the
//! service is busy and asserts the wrong thing when the data changes.

// The panic lints exist to keep shipped code free of panics on external input.
// A poisoned lock in a test double means a test panicked while holding it, so
// the test is already failing and the second panic says nothing new.
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
// `clippy::unused_async_trait_impl` wants a trait-impl method with no `.await` rewritten to
// return `impl Future`; `clippy::manual_async_fn` then wants that block turned back into an
// `async fn`. They cannot both be obeyed, which is what this expectation is about. The
// `async fn` spelling is kept because the trait itself is written that way, and because the
// fix clippy asks for would run the recording at call time rather than at first poll.
#![expect(
    clippy::unused_async_trait_impl,
    reason = "contradicts clippy::manual_async_fn, which wants the async fn back"
)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use bytes::Bytes;

use crate::{ChunkFuture, ChunkedBody, FetchError, Http, HttpResponse, ResponseBody};

/// A canned reply. A status of 0 stands for "the request never arrived", which
/// the transport has to be able to tell apart from a rejection.
#[derive(Clone, Debug)]
struct Fake {
    status: u16,
    body: Arc<str>,
}

/// A transport that answers each request from a script, and records what was
/// asked.
///
/// The script is consumed in order, so a test states the whole sequence of
/// calls it expects. Running out of replies is itself an assertion: the last
/// fake returned is an empty `200`, which fails to parse rather than quietly
/// answering.
#[derive(Clone, Debug)]
pub struct Scripted {
    replies: Arc<Mutex<VecDeque<Fake>>>,
    seen: Arc<Mutex<Vec<String>>>,
    /// Header names seen per call, so a test can assert that identification
    /// actually reaches the wire rather than being built and dropped.
    headers: Arc<Mutex<Vec<Vec<String>>>>,
    /// The body exactly as it was sent, before [`Scripted::queries`] decodes it.
    raw: Arc<Mutex<Vec<String>>>,
    /// The largest piece each body is handed over in. Zero means one piece.
    chunk_size: usize,
}

impl Scripted {
    /// Answer the given replies, in order, as `(status, body)`.
    #[must_use]
    pub fn new(replies: Vec<(u16, &str)>) -> Self {
        Self::with_chunk_size(replies, 0)
    }

    /// Answer the given replies, in order, handing each body over in pieces of at
    /// most `size` bytes.
    ///
    /// A size of zero means one piece. Anything else simulates a body that
    /// arrives in pieces, which is the case the streaming path exists for and the
    /// one a whole-body double cannot test: a payload that is only ever read in
    /// one go proves nothing about a reader that has to carry a record across the
    /// boundary.
    #[must_use]
    pub fn with_chunk_size(replies: Vec<(u16, &str)>, size: usize) -> Self {
        Self {
            chunk_size: size,
            replies: Arc::new(Mutex::new(
                replies
                    .into_iter()
                    .map(|(status, body)| Fake {
                        status,
                        body: Arc::from(body),
                    })
                    .collect(),
            )),
            seen: Arc::new(Mutex::new(Vec::new())),
            headers: Arc::new(Mutex::new(Vec::new())),
            raw: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Replace every reply from `index` on with the same one, so a retry loop
    /// the test did not intend cannot silently consume a later entry.
    ///
    /// # Panics
    /// If a test panicked while holding the script, which means that test has
    /// already failed and this one only reports the same problem again.
    pub fn then_always_from(&self, index: usize, status: u16, body: &str) {
        let mut replies = self.replies.lock().expect("not poisoned");
        replies.truncate(index);
        replies.extend(std::iter::repeat_n(
            Fake {
                status,
                body: Arc::from(body),
            },
            16,
        ));
    }

    /// The next scripted reply, or an empty `200` once the script is spent.
    ///
    /// An empty `200` fails to parse rather than answering, so a test that
    /// expected a fourth call and only scripted three is a failure rather than
    /// a missing match.
    fn reply(&self) -> Fake {
        self.replies
            .lock()
            .expect("not poisoned")
            .pop_front()
            .unwrap_or_else(|| Fake {
                status: 200,
                body: Arc::from(""),
            })
    }

    /// Record a call, so a wrapping transport can log what it forwards.
    ///
    /// The body is form-decoded, which is wrong for a JSON body and only affects
    /// what a test can assert on: `%` is not a character JSON uses.
    ///
    /// # Panics
    /// If a test panicked while holding the script, which means that test has
    /// already failed and this one only reports the same problem again.
    pub fn record(&self, endpoint: &str, body: &str) {
        self.raw
            .lock()
            .expect("not poisoned")
            .push(body.to_string());
        self.seen
            .lock()
            .expect("not poisoned")
            .push(format!("{endpoint}|{}", decode(body)));
    }

    /// Every request body exactly as it was sent, in order.
    ///
    /// [`Scripted::queries`] decodes the form and shows the `query` field, which
    /// is the useful view for asserting *what was asked*. This is the view for
    /// asserting what else travelled with it: a `timeout`, an `action`, anything
    /// that is not the question.
    /// # Panics
    #[must_use]
    pub fn raw_bodies(&self) -> Vec<String> {
        self.raw.lock().expect("not poisoned").clone()
    }

    /// Every call as `endpoint|query`, in order.
    ///
    /// Asserting on this is how a test checks that the *right* question was
    /// asked, not only that the right answer was read out of one.
    /// # Panics
    #[must_use]
    pub fn queries(&self) -> Vec<String> {
        self.seen.lock().expect("not poisoned").clone()
    }

    /// How many requests were made, retries included.
    /// # Panics
    #[must_use]
    pub fn call_count(&self) -> usize {
        self.seen.lock().expect("not poisoned").len()
    }

    /// The header names each call carried, in order, one `Vec` per call.
    ///
    /// Names only, not values: a test asserts *that* the client identifies
    /// itself, and asserting on a token's value would put a secret in a test.
    /// # Panics
    #[must_use]
    pub fn header_names(&self) -> Vec<Vec<String>> {
        self.headers.lock().expect("not poisoned").clone()
    }

    /// Which service each call went to, in order.
    /// # Panics
    #[must_use]
    pub fn endpoints(&self) -> Vec<String> {
        self.queries()
            .iter()
            .map(|call| call.split('|').next().unwrap_or_default().to_owned())
            .collect()
    }
}

impl Http for Scripted {
    type Response = ScriptedResponse;

    async fn post(
        &self,
        endpoint: &str,
        _accept: &str,
        body: String,
    ) -> Result<Self::Response, FetchError> {
        self.record(endpoint, &body);

        let reply = self.reply();
        if reply.status == 0 {
            return Err(FetchError::Network("connection refused".into()));
        }
        Ok(ScriptedResponse {
            fake: reply,
            chunk_size: self.chunk_size,
        })
    }

    async fn post_form(
        &self,
        endpoint: &str,
        accept: &str,
        body: String,
        headers: &[(&str, String)],
    ) -> Result<Self::Response, FetchError> {
        self.headers.lock().expect("not poisoned").push(
            headers
                .iter()
                .map(|(name, _)| (*name).to_string())
                .collect(),
        );
        self.post(endpoint, accept, body).await
    }

    async fn post_json(&self, url: &str, body: String) -> Result<Self::Response, FetchError> {
        self.record(url, &body);

        let reply = self.reply();
        if reply.status == 0 {
            return Err(FetchError::Network("connection refused".into()));
        }
        Ok(ScriptedResponse {
            fake: reply,
            chunk_size: self.chunk_size,
        })
    }

    async fn get(&self, url: &str, accept: &str) -> Result<Self::Response, FetchError> {
        self.record(url, accept);

        let reply = self.reply();
        if reply.status == 0 {
            return Err(FetchError::Network("connection refused".into()));
        }
        Ok(ScriptedResponse {
            fake: reply,
            chunk_size: self.chunk_size,
        })
    }
}

/// The reply [`Scripted`] hands back.
#[derive(Debug)]
pub struct ScriptedResponse {
    /// The scripted reply.
    fake: Fake,
    /// The largest piece the body is handed over in.
    chunk_size: usize,
}

impl HttpResponse for ScriptedResponse {
    fn status(&self) -> u16 {
        self.fake.status
    }

    async fn bytes(self) -> Result<ResponseBody, FetchError> {
        Ok(Bytes::from(self.fake.body.as_bytes().to_vec()))
    }

    async fn text(self) -> Result<String, FetchError> {
        Ok(self.fake.body.to_string())
    }

    async fn chunk(&mut self) -> Result<Option<ResponseBody>, FetchError> {
        Ok(None)
    }

    fn into_chunks(self) -> Result<ChunkedBody, FetchError> {
        let body = Bytes::from(self.fake.body.as_bytes().to_vec());
        let size = if self.chunk_size == 0 {
            body.len().max(1)
        } else {
            self.chunk_size
        };
        Ok(Box::new(ScriptedChunks { body, size, at: 0 }))
    }
}

/// A scripted body, handed over in pieces of `size` bytes.
struct ScriptedChunks {
    /// The whole body.
    body: Bytes,
    /// The largest piece to hand over at a time.
    size: usize,
    /// How much has been handed over.
    at: usize,
}

impl crate::BodyChunks for ScriptedChunks {
    fn next_chunk(&mut self) -> ChunkFuture<'_> {
        Box::pin(async move {
            if self.at >= self.body.len() {
                return Ok(None);
            }
            let end = self.at.saturating_add(self.size).min(self.body.len());
            let piece = self.body.slice(self.at..end);
            self.at = end;
            Ok(Some(piece))
        })
    }
}

/// Undo the form encoding so a recorded query can be asserted on directly.
///
/// A hand-rolled decoder, because the alternative is a URL-decoding dependency
/// in the test tree to save six lines here.
fn decode(body: &str) -> String {
    let encoded = body.strip_prefix("query=").unwrap_or(body).as_bytes();
    let mut out = Vec::with_capacity(encoded.len());
    let mut i = 0;
    while i < encoded.len() {
        let hex = match encoded.get(i..i + 3) {
            Some([b'%', hi, lo]) => char::from(*hi)
                .to_digit(16)
                .zip(char::from(*lo).to_digit(16))
                .map(|(hi, lo)| u8::try_from(hi * 16 + lo).expect("two hex digits fit a byte")),
            _ => None,
        };
        if let Some(byte) = hex {
            out.push(byte);
            i += 3;
        } else {
            out.push(if encoded[i] == b'+' { b' ' } else { encoded[i] });
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

#[cfg(test)]
#[path = "testing/tests.rs"]
mod tests;
