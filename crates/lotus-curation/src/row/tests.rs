// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Tests for [`CurationStatus`].

use super::CurationStatus;

#[test]
fn curation_status_as_key_maps_every_variant_distinctly() {
    assert_eq!(
        CurationStatus::ExistingComplete.as_key(),
        "existing_complete"
    );
    assert_eq!(
        CurationStatus::ExistingNeedsUpdates.as_key(),
        "existing_updates"
    );
    assert_eq!(CurationStatus::NewCompound.as_key(), "new_compound");
    assert_eq!(
        CurationStatus::PendingDependencies.as_key(),
        "pending_dependencies"
    );
    assert_eq!(CurationStatus::NotChecked.as_key(), "not_checked");
    assert_eq!(CurationStatus::Error.as_key(), "error");
}
