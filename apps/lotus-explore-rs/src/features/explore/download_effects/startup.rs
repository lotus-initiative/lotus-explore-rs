// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::download::DownloadFormat;

/// Check if startup effect should trigger based on download state and search history.
/// Returns `true` when all preconditions are met:
#[must_use]
pub const fn should_trigger_startup_search(
    pending_format: Option<DownloadFormat>,
    direct_execute: bool,
    searched_once: bool,
    loading: bool,
) -> bool {
    (pending_format.is_some() || direct_execute) && !searched_once && !loading
}

/// Determine telemetry call for startup trigger based on mode.
pub enum StartupTriggerMode {
    /// Format pending — user requested download startup.
    Download { format: DownloadFormat },
    /// Direct execute — user requested immediate search.
    DirectExecute,
}

impl StartupTriggerMode {
    /// Report startup trigger via appropriate telemetry channel.
    pub fn log(&self) {
        use crate::services::search_telemetry as telemetry;
        match self {
            Self::Download { format } => {
                telemetry::download_startup_auto_search_triggered(format.log_name());
            }
            Self::DirectExecute => {
                telemetry::search_startup_auto_search_execute();
            }
        }
    }
}
