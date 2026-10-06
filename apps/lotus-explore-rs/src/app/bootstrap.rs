// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Pure app bootstrap state assembly.

use crate::app_state::{AppState, DownloadState};
use crate::features::explore::{ExploreState, InitialUrlState};
use crate::i18n::Locale;
use lotus_search::SearchCriteria;

// Name mirrors the Dioxus `App{…}` component it feeds; `AppBootstrap` reads
// naturally and renaming would obscure the shared `App` prefix convention.
#[derive(Clone, PartialEq)]
pub struct AppBootstrap {
    pub app_state: AppState,
    pub criteria: SearchCriteria,
    pub criteria_baseline: SearchCriteria,
    pub locale: Locale,
    pub explore: ExploreState,
}

pub fn bootstrap_app(startup: InitialUrlState) -> AppBootstrap {
    let criteria = startup.criteria;
    let criteria_baseline = criteria.clone();

    AppBootstrap {
        app_state: AppState {
            download: DownloadState {
                pending_format: startup.download.pending_format,
                pending_invalid_format: startup.download.pending_invalid_format,
                direct_execute: startup.download.direct_execute,
            },
            dark_mode: startup.dark_mode,
            ..AppState::default()
        },
        criteria,
        criteria_baseline,
        locale: startup.locale,
        explore: ExploreState::default(),
    }
}

#[cfg(test)]
#[path = "bootstrap/tests.rs"]
mod tests;
