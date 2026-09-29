// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The transport seam: two methods, so that everything above can be tested
//! without a network.

use crate::error::FetchError;

/// A response body, not yet decoded.
pub type ResponseBody = bytes::Bytes;

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
}
