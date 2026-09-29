// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

#![allow(clippy::uninlined_format_args)]
#![allow(clippy::panic)]

use super::dispatch::{DispatchPhase, classify_dispatch_phase};
use super::metrics::{metrics_for_waiting_loading_phase, metrics_for_waiting_query_phase};
use super::startup::{StartupTriggerMode, should_trigger_startup_search};
use crate::app_state::MetricsState;
use crate::download::DownloadFormat;
use crate::features::explore::search_state::ExploreState;
use crate::models::SearchCriteria;

#[test]
fn should_trigger_startup_search_requires_all_conditions() {
    assert!(should_trigger_startup_search(
        Some(DownloadFormat::Csv),
        false,
        false,
        false,
    ));
    assert!(!should_trigger_startup_search(None, false, false, false));
    assert!(!should_trigger_startup_search(
        Some(DownloadFormat::Csv),
        false,
        true,
        false,
    ));
    assert!(!should_trigger_startup_search(
        Some(DownloadFormat::Csv),
        false,
        false,
        true,
    ));
}

#[test]
fn dispatch_phase_inactive_when_no_pending_format() {
    let phase = classify_dispatch_phase(None, &ExploreState::default());
    assert_eq!(phase, DispatchPhase::Inactive);
}

#[test]
fn dispatch_phase_waiting_for_loading_when_loading() {
    let mut explore = ExploreState::default();
    explore.lifecycle.loading = true;
    let phase = classify_dispatch_phase(Some(DownloadFormat::Csv), &explore);
    assert_eq!(
        phase,
        DispatchPhase::WaitingForLoading {
            format: DownloadFormat::Csv,
        }
    );
}

#[test]
fn dispatch_phase_waiting_for_query_when_query_absent() {
    let explore = ExploreState::default();
    let phase = classify_dispatch_phase(Some(DownloadFormat::Csv), &explore);
    assert_eq!(
        phase,
        DispatchPhase::WaitingForQuery {
            format: DownloadFormat::Csv,
        }
    );
}

#[test]
fn dispatch_phase_ready_when_all_preconditions_met() {
    let mut explore = ExploreState::default();
    explore.result.sparql_query = Some(std::sync::Arc::from("SELECT * WHERE {}"));
    explore.ui.executed_criteria = SearchCriteria {
        taxon: "Rosa".into(),
        ..SearchCriteria::up_to_year(crate::models::current_year())
    };

    let phase = classify_dispatch_phase(Some(DownloadFormat::Json), &explore);

    if let DispatchPhase::Ready {
        #[cfg(target_arch = "wasm32")]
        criteria,
        query,
        format,
        ..
    } = phase
    {
        #[cfg(target_arch = "wasm32")]
        assert_eq!(criteria.taxon, "Rosa");
        assert!(query.contains("SELECT"));
        assert_eq!(format, DownloadFormat::Json);
    } else {
        panic!("expected Ready phase, got {:?}", phase);
    }
}

#[test]
fn startup_trigger_mode_discriminates_by_source() {
    let download_mode = StartupTriggerMode::Download {
        format: DownloadFormat::Csv,
    };
    let execute_mode = StartupTriggerMode::DirectExecute;

    assert!(matches!(download_mode, StartupTriggerMode::Download { .. }));
    assert!(matches!(execute_mode, StartupTriggerMode::DirectExecute));
}

#[test]
fn waiting_loading_phase_sets_loading_guard_and_clears_query_guard() {
    let metrics = MetricsState {
        waiting_loading_logged: false,
        waiting_query_logged: true,
    };
    let next = metrics_for_waiting_loading_phase(&metrics, true);
    assert!(next.waiting_loading_logged);
    assert!(!next.waiting_query_logged);
}

#[test]
fn waiting_query_phase_sets_query_guard_and_clears_loading_guard() {
    let metrics = MetricsState {
        waiting_loading_logged: true,
        waiting_query_logged: false,
    };
    let next = metrics_for_waiting_query_phase(&metrics, true);
    assert!(!next.waiting_loading_logged);
    assert!(next.waiting_query_logged);
}
