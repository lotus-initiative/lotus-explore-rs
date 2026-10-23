// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::{QueryParams, is_true_flag};
use lotus_query::ExportFormat as DownloadFormat;

/// A download or execute action requested by the startup URL.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InitialDownloadState {
    /// The format to download in, when a valid format was requested.
    pub pending_format: Option<DownloadFormat>,
    /// The invalid format string that was requested, if any.
    pub pending_invalid_format: Option<String>,
    /// Whether the search should execute immediately.
    pub direct_execute: bool,
}

/// Decode the startup download or execute action from URL parameters.
#[must_use]
pub fn parse_startup_action_from_params(params: &QueryParams) -> InitialDownloadState {
    if !params.get("download").is_some_and(|v| is_true_flag(v)) {
        return InitialDownloadState {
            direct_execute: params.get("execute").is_some_and(|v| is_true_flag(v)),
            ..InitialDownloadState::default()
        };
    }

    let requested_format = params.get("format").map_or("csv", String::as_str);
    let pending_format = DownloadFormat::parse(requested_format);

    InitialDownloadState {
        pending_format,
        pending_invalid_format: pending_format
            .is_none()
            .then(|| requested_format.trim().to_ascii_lowercase()),
        direct_execute: false,
    }
}
