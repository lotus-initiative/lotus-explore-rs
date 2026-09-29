// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Pure app bootstrap state assembly.

use crate::app_state::{AppState, DownloadState};
use crate::features::explore::{ExploreState, InitialUrlState};
use crate::i18n::Locale;
use crate::models::SearchCriteria;

// Name mirrors the Dioxus `App{…}` component it feeds; `AppBootstrap` reads
// naturally and renaming would obscure the shared `App` prefix convention.
#[allow(clippy::module_name_repetitions)]
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
mod tests {
    use super::*;
    use crate::download::DownloadFormat;
    use crate::features::explore::InitialDownloadState;

    #[test]
    fn bootstrap_app_copies_startup_locale_and_download_state() {
        let startup = InitialUrlState {
            criteria: SearchCriteria::default(),
            locale: Locale::Fr,
            download: InitialDownloadState {
                pending_format: Some(DownloadFormat::Csv),
                pending_invalid_format: Some("ttl".into()),
                direct_execute: true,
            },
            dark_mode: false,
        };

        let bootstrap = bootstrap_app(startup);
        assert_eq!(bootstrap.locale, Locale::Fr);
        assert_eq!(
            bootstrap.app_state.download.pending_format,
            Some(DownloadFormat::Csv)
        );
        assert_eq!(
            bootstrap
                .app_state
                .download
                .pending_invalid_format
                .as_deref(),
            Some("ttl")
        );
        assert!(bootstrap.app_state.download.direct_execute);
    }

    #[test]
    fn bootstrap_app_uses_initial_criteria_as_dirty_tracking_baseline() {
        let startup = InitialUrlState {
            criteria: SearchCriteria {
                taxon: "Rosa".into(),
                ..SearchCriteria::default()
            },
            locale: Locale::En,
            download: InitialDownloadState::default(),
            dark_mode: false,
        };

        let bootstrap = bootstrap_app(startup);
        assert_eq!(bootstrap.criteria, bootstrap.criteria_baseline);
        assert!(bootstrap.explore == ExploreState::default());
    }
}
