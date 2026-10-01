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
    /// Searched once, then the form was edited and not yet re-run.
    Stale,
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
    /// The search form has been edited since the last search ran.
    ///
    /// This is what stops the results from belonging to a query that is no longer
    /// the one on screen. Changing a field does not dispatch a search -- the user
    /// presses the button -- so without this the table and the stats cards sit
    /// there describing the *previous* criteria, under a form showing the new
    /// ones. Two panels on screen disagreeing about what is being searched is
    /// worse than either being absent.
    pub criteria_dirty: bool,
}

impl From<LifecycleBooleans> for ContentPhase {
    fn from(state: LifecycleBooleans) -> Self {
        let LifecycleBooleans {
            loading,
            has_error,
            searched_once,
            download_only_mode,
            has_entries,
            criteria_dirty,
        } = state;
        if loading {
            Self::Loading
        } else if has_error {
            Self::Error
        } else if download_only_mode {
            Self::DownloadOnly
        } else if !searched_once {
            Self::Welcome
        } else if criteria_dirty {
            // Edited but not yet re-run. Its own phase rather than `Empty`, so
            // the page can say why the results went away and what to do about it
            // instead of looking like the search found nothing.
            Self::Stale
        } else if !has_entries {
            Self::Empty
        } else {
            Self::Loaded
        }
    }
}
