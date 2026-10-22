// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The transport boundary: what the domain asks a host to do, and how it is
//! told when that fails.
//!
//! The domain never performs I/O. It asks a [`LotusRepository`] for a body and
//! receives either one or a [`RepositoryError`]; deciding what a failure means
//! — transient, permanent, or "the endpoint gave up on the query" — is done
//! once, here and in `explore::transport_classification`, so the retry policy,
//! the notice strip and the telemetry cannot disagree about the same error.

use crate::api::dto::SearchResponse;
use lotus_model::ColumnarResultSet;
use lotus_search::SearchCriteria;
#[cfg(not(target_arch = "wasm32"))]
use std::io::{Seek, Write};
use std::sync::Arc;
use thiserror::Error;

/// Why a transport call did not produce an answer.
///
/// The variants are the distinctions the rest of the app acts on. `Network` and
/// `Http` are not the same: one is worth retrying and the other usually is
/// not, and `Truncated` is neither — the answer arrived, and it was short.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum RepositoryError {
    /// No API base URL is configured, so the fast path cannot be attempted.
    #[error("LOTUS API not configured")]
    NotConfigured,

    /// The request never reached the endpoint.
    #[error("network error: {0}")]
    Network(Arc<str>),

    /// The endpoint answered with a non-success status.
    #[error("HTTP {status}: {body}")]
    Http {
        /// The status the endpoint answered with.
        status: u16,
        /// The body it answered with.
        body: String,
    },

    /// The answer arrived and could not be read.
    #[error("parse error: {0}")]
    Parse(Arc<str>),

    /// A result set that stopped short of what the query asked for.
    ///
    /// Its own variant rather than [`RepositoryError::Network`] so a caller can
    /// tell "nothing came" from "some came, which is not all of it" — collapsing
    /// the two makes a search report the row count of whatever bytes survived.
    #[error("incomplete result set: {0}")]
    Truncated(Arc<str>),
}

impl RepositoryError {
    /// A transport failure that never reached the endpoint.
    #[must_use]
    pub fn network(message: impl Into<Arc<str>>) -> Self {
        Self::Network(message.into())
    }

    /// An answer that arrived and could not be read.
    #[must_use]
    pub fn parse(message: impl Into<Arc<str>>) -> Self {
        Self::Parse(message.into())
    }

    /// An answer that arrived and was short.
    #[must_use]
    pub fn truncated(message: impl Into<Arc<str>>) -> Self {
        Self::Truncated(message.into())
    }
}

impl From<crate::api::error::ApiClientError> for RepositoryError {
    fn from(value: crate::api::error::ApiClientError) -> Self {
        match value {
            crate::api::error::ApiClientError::Network(msg) => Self::network(msg),
            crate::api::error::ApiClientError::Http(status, body) => Self::Http { status, body },
            crate::api::error::ApiClientError::Parse(msg) => Self::parse(msg),
        }
    }
}

/// Boundary trait for data-access operations used by the search orchestrator.
/// Implementations may delegate to the REST API, SPARQL, or a test stub.
// The futures this trait hands back are not `Send`, and cannot be: the search
// pipeline that drives them holds a UI signal, which is a `RefCell`. Desugaring
// to `impl Future` would let a caller *ask* for `Send`, and the answer would
// still be no.
#[allow(
    async_fn_in_trait,
    reason = "the pipeline's futures hold a `RefCell`-backed signal and are never `Send`"
)]
pub trait LotusRepository: Clone + 'static {
    /// Try the REST API fast path.  Returns:
    async fn api_search(
        &self,
        criteria: &SearchCriteria,
        limit: usize,
        include_counts: bool,
    ) -> Option<Result<SearchResponse, RepositoryError>>;

    /// Execute a SPARQL query and return the raw response body.
    async fn sparql_body(&self, query: &str)
    -> Result<lotus_search::ResponseBody, RepositoryError>;

    /// Execute a result query and fold the whole answer into a columnar set.
    ///
    /// The interactive path: unlike [`Self::sparql_body`] it never assembles the
    /// body. A result set too large for memory cannot be fetched by a method that
    /// holds it in memory first, and the widest search measures 2,990,730 edges
    /// at the 314 B/row a real export averages — about 940 MB of decompressed
    /// CSV — against a 200 MB budget.
    ///
    /// `query` must be the *unlimited* query: a `LIMIT` here puts the old
    /// truncation back server-side, where nothing downstream could undo it.
    ///
    /// The default refuses rather than falling back to [`Self::sparql_body`],
    /// because the fallback looks like it worked and exhausts memory later.
    ///
    /// `on_progress` fires once per chunk read, so a caller that re-renders on
    /// every call must throttle — see
    /// [`lotus_search::columnar_from_chunks_reporting`].
    ///
    /// # Errors
    /// Returns [`RepositoryError::Network`] if this transport cannot read a body
    /// incrementally.
    async fn sparql_columnar<F>(
        &self,
        query: &str,
        on_progress: &mut F,
    ) -> Result<ColumnarResultSet, RepositoryError>
    where
        F: FnMut(lotus_search::StreamProgress) + ?Sized,
    {
        let _ = (query, on_progress);
        Err(RepositoryError::network(
            "this transport cannot read a response body in chunks",
        ))
    }

    /// Execute a result query and spool the whole answer to a temporary file.
    ///
    /// The native path for an export too large to hold in memory: the body
    /// goes to disk and the caller reads it back in chunks.
    #[cfg(not(target_arch = "wasm32"))]
    async fn sparql_tempfile(
        &self,
        query: &str,
    ) -> Result<tempfile::NamedTempFile, RepositoryError> {
        let body = self.sparql_body(query).await?;
        let mut file = tempfile::NamedTempFile::new()
            .map_err(|e| RepositoryError::parse(format!("tempfile create failed: {e}")))?;
        file.write_all(&body)
            .map_err(|e| RepositoryError::parse(format!("tempfile write failed: {e}")))?;
        file.as_file_mut()
            .rewind()
            .map_err(|e| RepositoryError::parse(format!("tempfile rewind failed: {e}")))?;
        Ok(file)
    }
}
