// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `command`, in their own file.

use super::SearchCommand;

#[test]
fn startup_download_maps_to_direct_download_only() {
    assert!(!SearchCommand::Interactive.direct_download());
    assert!(!SearchCommand::StartupExecute.direct_download());
    assert!(SearchCommand::StartupDownload.direct_download());
}
