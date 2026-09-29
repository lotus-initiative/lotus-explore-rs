// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Typed commands for search entry points.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchCommand {
    /// User clicked search / pressed Enter / requested preview.
    Interactive,
    /// App boot requested immediate execution from URL params.
    StartupExecute,
    /// App boot requested download-mode execution from URL params.
    StartupDownload,
}

impl SearchCommand {
    #[must_use]
    pub const fn direct_download(self) -> bool {
        matches!(self, Self::StartupDownload)
    }
}

#[cfg(test)]
mod tests {
    use super::SearchCommand;

    #[test]
    fn startup_download_maps_to_direct_download_only() {
        assert!(!SearchCommand::Interactive.direct_download());
        assert!(!SearchCommand::StartupExecute.direct_download());
        assert!(SearchCommand::StartupDownload.direct_download());
    }
}
