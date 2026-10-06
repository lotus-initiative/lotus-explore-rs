// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::{SINK_PREFERENCE, SinkPreference};

#[test]
fn memory_is_the_last_resort() {
    // `Blob` is the variant that assembles the whole export in the tab's heap, and
    // that is what crashed the tab at full size. It must be reachable only when
    // neither file sink is.
    assert_eq!(
        SINK_PREFERENCE.last(),
        Some(&SinkPreference::Memory),
        "the in-memory sink must be the final fallback, not the first choice"
    );
}

#[test]
fn the_reader_chosen_file_is_preferred_over_private_storage() {
    // Where both exist the reader picks the location and no temporary file is
    // involved. Falling through to OPFS on Chromium would add a temp file and a
    // delete for no gain.
    assert_eq!(SINK_PREFERENCE[0], SinkPreference::UserChosenFile);
    assert!(
        SINK_PREFERENCE
            .iter()
            .position(|s| *s == SinkPreference::PrivateStorage)
            < SINK_PREFERENCE
                .iter()
                .position(|s| *s == SinkPreference::Memory),
        "private storage must be tried before falling back to memory"
    );
}

#[test]
fn every_preference_is_distinct() {
    let mut seen = SINK_PREFERENCE.to_vec();
    seen.sort_by_key(|s| format!("{s:?}"));
    seen.dedup();
    assert_eq!(
        seen.len(),
        SINK_PREFERENCE.len(),
        "a duplicated preference means one branch can never be reached"
    );
}
