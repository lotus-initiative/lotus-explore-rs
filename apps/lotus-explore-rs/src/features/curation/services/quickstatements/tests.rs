// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `quickstatements`, in their own file.

use super::*;

#[test]
fn falls_back_to_home_for_empty_commands() {
    assert_eq!(build_qs_dev_link("   "), QS_DEV_HOME);
}

#[test]
fn builds_batch_link_for_commands() {
    let url = build_qs_dev_link("CREATE\nLAST|Len|\"A\"");
    assert!(url.starts_with(QS_DEV_BATCH_NEW));
    assert!(url.contains("CREATE%0ALAST%7CLen%7C%22A%22"));
}
