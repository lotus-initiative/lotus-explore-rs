// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Typed errors for browser and native export downloads.

/// Why a download or export operation failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DownloadError {
    /// Invalid download input (non-HTTP URL, malformed PNG data URL, or invalid base64).
    #[cfg(not(target_arch = "wasm32"))]
    #[error("{0}")]
    InvalidInput(String),
    /// Filesystem or process-launch failure on desktop builds.
    #[cfg(not(target_arch = "wasm32"))]
    #[error("{0}")]
    Io(String),
    /// Network or upstream HTTP/SPARQL failure.
    #[error("{0}")]
    Network(String),
    /// Browser storage, file picker, or DOM/Blob failure on wasm builds.
    #[cfg(target_arch = "wasm32")]
    #[error("{0}")]
    Browser(String),
    /// Export exceeds the in-memory Blob ceiling when no streaming file sink is available.
    #[cfg(target_arch = "wasm32")]
    #[error("{}", super::blob_path_message(*rows, *limit))]
    InMemoryLimit {
        /// Number of rows requested.
        rows: usize,
        /// Format-specific in-memory row cap.
        limit: usize,
    },
}

#[cfg(not(target_arch = "wasm32"))]
impl DownloadError {
    /// Whether this error indicates an upstream 502/gateway failure suitable for WDQS fallback.
    #[must_use]
    pub(super) fn is_gateway_error(&self) -> bool {
        matches!(
            self,
            Self::Network(msg)
                if msg.contains("502") || msg.contains("Bad Gateway") || msg.contains("gateway")
        )
    }
}

#[cfg(target_arch = "wasm32")]
impl From<crate::upload::UploadError> for DownloadError {
    fn from(err: crate::upload::UploadError) -> Self {
        Self::Browser(err.to_string())
    }
}
