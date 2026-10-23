// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Typed commands for search entry points.

/// The entry point that asked for a search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SearchCommand {
    /// User clicked search / pressed Enter / requested preview.
    Interactive,
    /// App boot requested immediate execution from URL params.
    StartupExecute,
    /// App boot requested download-mode execution from URL params.
    StartupDownload,
}

impl SearchCommand {
    /// Whether this command prepares a download instead of an interactive search.
    #[must_use]
    #[allow(dead_code)]
    pub const fn direct_download(self) -> bool {
        matches!(self, Self::StartupDownload)
    }
}

#[cfg(test)]
#[path = "command/tests.rs"]
mod tests;
