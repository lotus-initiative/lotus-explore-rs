// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Repository layer: a thin boundary between search orchestration and transport.

pub mod hybrid;
#[cfg(test)]
pub mod mock;

// Re-export WDQS fallback tracking functions from hybrid.rs
pub use hybrid::{get_wdqs_transformed_query, is_wdqs_fallback_used, reset_wdqs_fallback_flag};

pub use hybrid::HybridRepository;

use crate::api::SearchResponse;
use lotus_model::ColumnarResultSet;
use lotus_search::SearchCriteria;
#[cfg(not(target_arch = "wasm32"))]
use std::io::{Seek, Write};
use std::sync::Arc;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum RepositoryError {
    #[error("LOTUS API not configured")]
    NotConfigured,

    #[error("network error: {0}")]
    Network(Arc<str>),

    #[error("HTTP {status}: {body}")]
    Http { status: u16, body: String },

    #[error("parse error: {0}")]
    Parse(Arc<str>),
}

impl RepositoryError {
    pub fn network(message: impl Into<Arc<str>>) -> Self {
        Self::Network(message.into())
    }

    pub fn parse(message: impl Into<Arc<str>>) -> Self {
        Self::Parse(message.into())
    }
}

impl From<crate::api::ApiClientError> for RepositoryError {
    fn from(value: crate::api::ApiClientError) -> Self {
        match value {
            crate::api::ApiClientError::Network(msg) => Self::network(msg),
            crate::api::ApiClientError::Http(status, body) => Self::Http { status, body },
            crate::api::ApiClientError::Parse(msg) => Self::parse(msg),
        }
    }
}

/// Boundary trait for data-access operations used by the search orchestrator.
/// Implementations may delegate to the REST API, SPARQL, or a test stub.
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
    /// This is the interactive path. It differs from [`Self::sparql_body`] in the
    /// one way that matters: the body is never assembled. A result set that does
    /// not fit in memory cannot be fetched by a method that holds it in memory
    /// first, and the widest search measures about 470 MB of decompressed CSV
    /// against a 200 MB budget.
    ///
    /// `query` must be the *unlimited* query. A `LIMIT` here would put the old
    /// truncation back, server-side, where nothing downstream could undo it.
    ///
    /// The default refuses rather than silently falling back to
    /// [`Self::sparql_body`]: a transport that cannot stream has to say so,
    /// because the alternative looks like it worked and exhausts memory later.
    ///
    /// # Errors
    /// Returns [`RepositoryError::Network`] if this transport cannot read a body
    /// incrementally.
    async fn sparql_columnar(&self, query: &str) -> Result<ColumnarResultSet, RepositoryError> {
        let _ = query;
        Err(RepositoryError::network(
            "this transport cannot read a response body in chunks",
        ))
    }

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
