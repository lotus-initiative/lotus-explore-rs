// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Common UI utilities and phase models.

/// High-level lifecycle phase for the results area viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentPhase {
    Welcome,
    Loading,
    Error,
    DownloadOnly,
    Empty,
    Loaded,
}

// The booleans are independent UI flags read by separate components; packing
// them into a state-machine enum would couple unrelated rendering concerns.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LifecycleBooleans {
    pub loading: bool,
    pub has_error: bool,
    pub searched_once: bool,
    pub download_only_mode: bool,
    pub has_entries: bool,
}

impl From<LifecycleBooleans> for ContentPhase {
    fn from(state: LifecycleBooleans) -> Self {
        let LifecycleBooleans {
            loading,
            has_error,
            searched_once,
            download_only_mode,
            has_entries,
        } = state;
        if loading {
            Self::Loading
        } else if has_error {
            Self::Error
        } else if download_only_mode {
            Self::DownloadOnly
        } else if !searched_once {
            Self::Welcome
        } else if !has_entries {
            Self::Empty
        } else {
            Self::Loaded
        }
    }
}
