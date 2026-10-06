// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Consolidated app-level state for download orchestration and render telemetry.

use lotus_query::ExportFormat as DownloadFormat;

/// App-level state.  One signal of this type lives at the root of `App`.
/// Scope is deliberately narrow:
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct AppState {
    /// Download orchestration (format pending, direct-execute mode).
    pub download: DownloadState,

    /// One-shot logging guards used by the download-dispatch hook.
    pub metrics: MetricsState,

    /// Dark mode preference.
    pub dark_mode: bool,
}

/// Download action orchestration and pending-format queue.
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct DownloadState {
    /// Parsed pending programmatic download format.
    pub pending_format: Option<DownloadFormat>,

    /// Raw invalid format from URL (`?download=true&format=...`) preserved so
    /// startup validation can report the exact unsupported value once.
    pub pending_invalid_format: Option<String>,

    /// `true` when the URL included `?execute=true` (direct search + preview).
    pub direct_execute: bool,
}

/// Guards that prevent duplicate log events during the download-wait sequence.
/// These are reset to `false` once the awaited condition resolves.
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct MetricsState {
    /// We already logged "waiting for loading to finish" this dispatch cycle.
    pub waiting_loading_logged: bool,
    /// We already logged "waiting for SPARQL query to materialize" this cycle.
    pub waiting_query_logged: bool,
}

#[cfg(test)]
#[path = "app_state/tests.rs"]
mod tests;
