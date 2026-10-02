// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! The app's HTTP layer.
//!
//! `lotus-search` takes a transport trait and an endpoint value, so that its
//! retry and fallback logic can be tested against a scripted conversation with
//! no network. That is the right shape for a library and the wrong shape for an
//! application, where every call site wants to say "run this on that endpoint"
//! and not assemble a client first. These wrappers are that sentence, and they
//! are the only place the app names `reqwest`.
//!
//! They fail loudly rather than falling back, so choosing to try `WDQS` after a
//! failure stays the caller's decision. The two WDQS helpers at the bottom are
//! the exception: they resolve a query to the service that should answer it,
//! which is routing rather than transport.

use lotus_search::reqwest_client::ReqwestClient;
use lotus_search::{Endpoint, Service, execute_streaming};

pub use lotus_search::{
    FetchError, HttpResponse, QLEVER_WIKIDATA, ResponseBody, ResponseFormat, WDQS_SCHOLARLY,
    WDQS_WIKIDATA, execute,
};

// Reading the answer is part of talking to the endpoint, so these are reachable
// from here too. They are implemented in `lotus-query`.
pub use lotus_query::parse_taxon_csv;

/// The endpoint for a service name, so a caller can pass a URL it already had.
fn endpoint_for(url: &str) -> Endpoint {
    if url == WDQS_WIKIDATA {
        Endpoint::new(Service::Wdqs)
    } else if url == WDQS_SCHOLARLY {
        Endpoint::new(Service::Scholarly)
    } else {
        Endpoint::new(Service::Qlever)
    }
}

/// Run a query and hand back its body to be read a chunk at a time.
///
/// This is what the browser uses for a whole result set. The alternative --
/// [`execute_sparql_body`] -- assembles the payload first, and the widest search
/// measures about 470 MB of decompressed CSV, which is more than the module has.
///
/// # Errors
/// Propagates any transport or status failure. There is no empty-body check,
/// because finding out whether the body is empty means reading it.
pub async fn execute_sparql_chunks(sparql: &str) -> Result<lotus_search::ChunkedBody, FetchError> {
    let http = ReqwestClient::new()?;
    Ok(execute_streaming(
        &http,
        Endpoint::new(Service::Qlever),
        sparql,
        ResponseFormat::Csv,
    )
    .await?
    .chunks)
}

/// The streaming body against an explicit endpoint URL, for the WDQS fallback
/// and for exports in a format other than CSV.
///
/// # Errors
/// Propagates any transport or status failure.
pub async fn execute_sparql_chunks_at(
    sparql: &str,
    url: &str,
    format: ResponseFormat,
) -> Result<lotus_search::ChunkedBody, FetchError> {
    let http = ReqwestClient::new()?;
    Ok(execute_streaming(&http, endpoint_for(url), sparql, format)
        .await?
        .chunks)
}

// Only the native and server paths reach this; the browser client has its
// own fetch path, so a wasm build has no caller for it.
#[cfg(not(target_arch = "wasm32"))]
/// Run a query on `QLever` and return the body as text.
///
/// # Errors
/// Propagates any transport, status or decode failure, including after the
/// built-in retries are exhausted.
pub async fn execute_query(sparql: &str) -> Result<String, FetchError> {
    let http = ReqwestClient::new()?;
    let answer = execute(
        &http,
        Endpoint::new(Service::Qlever),
        sparql,
        ResponseFormat::Csv,
    )
    .await?;
    answer.text()
}

// Only the native and server paths reach this; the browser client has its
// own fetch path, so a wasm build has no caller for it.
#[cfg(not(target_arch = "wasm32"))]
/// Run a query on `QLever` and return the raw bytes, without decoding.
///
/// Used where the payload is an archive being streamed to disk or handed to the
/// browser untouched.
///
/// # Errors
/// Propagates any transport, status or decode failure.
pub async fn execute_sparql_bytes(sparql: &str) -> Result<Vec<u8>, FetchError> {
    let http = ReqwestClient::new()?;
    let answer = execute(
        &http,
        Endpoint::new(Service::Qlever),
        sparql,
        ResponseFormat::Csv,
    )
    .await?;
    Ok(answer.body)
}

/// Run a query on `QLever` and return the body undecoded.
///
/// # Errors
/// Propagates any transport, status or decode failure.
pub async fn execute_sparql_body(sparql: &str) -> Result<ResponseBody, FetchError> {
    let http = ReqwestClient::new()?;
    let answer = execute(
        &http,
        Endpoint::new(Service::Qlever),
        sparql,
        ResponseFormat::Csv,
    )
    .await?;
    Ok(answer.body.into())
}

// Only the native and server paths reach this; the browser client has its
// own fetch path, so a wasm build has no caller for it.
#[cfg(not(target_arch = "wasm32"))]
/// Run a query on `QLever`, asking for a specific representation.
///
/// # Errors
/// Propagates any transport, status or decode failure.
pub async fn execute_sparql_format(
    sparql: &str,
    format: ResponseFormat,
) -> Result<String, FetchError> {
    let http = ReqwestClient::new()?;
    execute(&http, Endpoint::new(Service::Qlever), sparql, format)
        .await?
        .text()
}

/// Run a query on a named endpoint and return the body undecoded.
///
/// The endpoint is given as a URL so that a caller which already chose
/// `WDQS` — because `QLever` was unreachable, or because the scholarly
/// properties are the point — does not have to map it back to a [`Service`].
///
/// # Errors
/// Propagates any transport, status or decode failure.
pub async fn execute_sparql_body_at(sparql: &str, url: &str) -> Result<ResponseBody, FetchError> {
    let http = ReqwestClient::new()?;
    let answer = execute(&http, endpoint_for(url), sparql, ResponseFormat::Csv).await?;
    Ok(answer.body.into())
}

/// Run a query and write the answer to a file, for an export.
///
/// # Errors
/// Propagates transport failures and any problem creating or writing the file.
#[cfg(not(target_arch = "wasm32"))]
pub async fn execute_sparql_tempfile_at(
    sparql: &str,
    url: &str,
) -> Result<tempfile::NamedTempFile, FetchError> {
    use std::io::Write as _;

    let http = ReqwestClient::new()?;
    let answer = execute(&http, endpoint_for(url), sparql, ResponseFormat::Csv).await?;
    let mut file = tempfile::NamedTempFile::new()
        .map_err(|e| FetchError::Network(format!("could not create a temporary file: {e}")))?;
    file.write_all(&answer.body)
        .map_err(|e| FetchError::Network(format!("could not write the export: {e}")))?;
    file.flush()
        .map_err(|e| FetchError::Network(format!("could not flush the export: {e}")))?;
    Ok(file)
}

// Only the native and server paths reach this; the browser client has its
// own fetch path, so a wasm build has no caller for it.
#[cfg(not(target_arch = "wasm32"))]
pub async fn execute_sparql_tempfile(sparql: &str) -> Result<tempfile::NamedTempFile, FetchError> {
    execute_sparql_tempfile_at(sparql, QLEVER_WIKIDATA).await
}

/// Rewrite a query for `WDQS`, returning the endpoint URL to send it to.
///
/// The download path needs the URL as a string because it is about to hand it
/// to a browser's navigation or to a redirect, not to a client. The routing
/// decision itself is `lotus_query::wdqs_fallback`'s; this resolves that service
/// to a URL and so respects the endpoint overrides.
#[must_use]
pub fn wdqs_download_query(query: &str) -> (&'static str, String) {
    let (service, rewritten) = lotus_query::wdqs_fallback(query);
    let url = match service {
        lotus_query::FallbackService::Scholarly => WDQS_SCHOLARLY,
        lotus_query::FallbackService::Main => WDQS_WIKIDATA,
    };
    (url, rewritten)
}

/// Rewrite a query so `WDQS` answers it, keeping the main endpoint.
#[must_use]
pub fn transform_query_for_wdqs(query: &str) -> String {
    wdqs_download_query(query).1
}

/// Run a query on a named endpoint, asking for a specific representation.
///
/// # Errors
/// Propagates any transport, status or decode failure.
pub async fn execute_sparql_format_at(
    sparql: &str,
    url: &str,
    format: ResponseFormat,
) -> Result<String, FetchError> {
    let http = ReqwestClient::new()?;
    let answer = execute(&http, endpoint_for(url), sparql, format).await?;
    answer.text()
}
