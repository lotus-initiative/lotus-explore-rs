// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The failure the REST client reports.

use thiserror::Error;

/// Why a REST call did not produce an answer.
#[derive(Debug, Error)]
pub enum ApiClientError {
    /// The request never reached the server.
    #[error("network error: {0}")]
    Network(String),

    /// The server answered with a non-success status.
    #[error("HTTP {0}: {1}")]
    Http(u16, String),

    /// The answer arrived and could not be read.
    #[error("parse error: {0}")]
    Parse(String),
}
