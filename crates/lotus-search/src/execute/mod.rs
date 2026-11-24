// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Running a query, and falling back when the endpoint is gone.
//!
//! Everything here is generic over [`Http`](crate::Http), so the retry and
//! fallback policy is testable with a scripted client and no network.

mod budget;
mod endpoints;
mod error_format;
mod fallback;
mod retry;
#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(test)]
mod tests;

pub use endpoints::{Answer, Endpoint, Service, StreamAnswer};
pub use fallback::{execute_streaming_with_fallback, execute_with_fallback, fetch_url};
pub use retry::{execute, execute_streaming};

#[cfg(test)]
use crate::{FetchError, ResponseFormat};
#[cfg(test)]
use budget::{
    ACCESS_TOKEN_HEADER, QLever, access_token, access_token_from, clamp_budget, query_budget_from,
    request_headers,
};
#[cfg(test)]
use error_format::{compact, html_title, json_exception, urlencode};
#[cfg(test)]
use fallback::is_success;
#[cfg(test)]
use lotus_model::{QLEVER_WIKIDATA, WDQS_SCHOLARLY, WDQS_WIKIDATA};
