// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `bootstrap`, in their own file.

use super::*;
use crate::features::explore::InitialDownloadState;
use lotus_query::ExportFormat as DownloadFormat;

#[test]
fn bootstrap_app_copies_startup_locale_and_download_state() {
    let startup = InitialUrlState {
        criteria: SearchCriteria::up_to_year(crate::clock::current_year()),
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
            ..SearchCriteria::up_to_year(crate::clock::current_year())
        },
        locale: Locale::En,
        download: InitialDownloadState::default(),
        dark_mode: false,
    };

    let bootstrap = bootstrap_app(startup);
    assert_eq!(bootstrap.criteria, bootstrap.criteria_baseline);
    assert!(bootstrap.explore == ExploreState::default());
}
