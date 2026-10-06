// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `app_services`, in their own file.

use super::*;

#[test]
fn app_services_repository_is_consistent() {
    let services = AppServices::new();
    let repo1 = services.repository();
    let repo2 = services.repository();
    assert_eq!(repo1, repo2);
}
