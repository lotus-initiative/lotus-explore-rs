// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Custom Dioxus hooks that encapsulate the download-related reactive effects.
//! These hooks coordinate two independent effect scenarios:
//! More detail in the type and function docs below.

use crate::app_state::{AppState, MetricsState};
use crate::download::execute_download;
use crate::features::explore::actions::ExploreAction;
use crate::features::explore::command::SearchCommand;
use crate::features::explore::download_effects::{self, DispatchPhase, StartupTriggerMode};
use crate::features::explore::orchestrator::{SearchTaskController, start_search};
use crate::features::explore::search_state::{ExploreState, dispatch_explore_action};
use crate::features::explore::types::{DomainError, ValidationFault};
use crate::models::SearchCriteria;
use crate::repositories::LotusRepository;
use crate::services::search_telemetry as telemetry;
use dioxus::prelude::*;

pub fn use_startup_effect<R: LotusRepository>(
    mut app_state: Signal<AppState>,
    explore: Signal<ExploreState>,
    criteria: Signal<SearchCriteria>,
    search_tasks: SearchTaskController,
    repo: R,
) {
    let repo_for_effect = repo;
    use_effect(move || {
        let repo = repo_for_effect.clone();
        let (pending, invalid_pending, direct_execute) = {
            let state = app_state.read();
            (
                state.download.pending_format,
                state.download.pending_invalid_format.clone(),
                state.download.direct_execute,
            )
        };
        if let Some(fmt) = invalid_pending {
            telemetry::download_startup_unsupported_format(&fmt);
            dispatch_explore_action(
                explore,
                ExploreAction::SearchFailed {
                    error: DomainError::Validation(ValidationFault::UnsupportedFormat {
                        format: fmt,
                    }),
                    query: None,
                },
            );
            app_state.with_mut(|state| {
                if state.download.pending_invalid_format.is_some() {
                    state.download.pending_invalid_format = None;
                }
            });
            return;
        }

        let (searched_once, loading) = {
            let explore_state = explore.read();
            (
                explore_state.lifecycle.searched_once,
                explore_state.lifecycle.loading,
            )
        };
        if download_effects::should_trigger_startup_search(
            pending,
            direct_execute,
            searched_once,
            loading,
        ) {
            let (trigger_mode, command) = pending.map_or(
                (
                    StartupTriggerMode::DirectExecute,
                    SearchCommand::StartupExecute,
                ),
                |format| {
                    (
                        StartupTriggerMode::Download { format },
                        SearchCommand::StartupDownload,
                    )
                },
            );
            trigger_mode.log();

            start_search(criteria, command, explore, search_tasks.clone(), repo);
            if direct_execute {
                app_state.with_mut(|state| state.download.direct_execute = false);
            }
        }
    });
}

pub fn use_download_dispatch_effect(
    mut app_state: Signal<AppState>,
    explore: Signal<ExploreState>,
) {
    use_effect(move || {
        let pending = {
            let state = app_state.read();
            state.download.pending_format
        };
        // Classify current phase based on pending format and explore state.
        // Compute within a scoped read to avoid cloning the entire ExploreState.
        let phase = {
            let state = explore.read();
            download_effects::classify_dispatch_phase(pending, &state)
        };

        match phase {
            DispatchPhase::Inactive => {
                // No download pending — reset any logging guards.
                let metrics = app_state.peek().metrics.clone();
                let next = MetricsState::default();
                if next != metrics {
                    app_state.with_mut(|state| state.metrics = next);
                }
            }
            DispatchPhase::WaitingForLoading { format } => {
                // Still loading — log once per cycle via guard.
                let metrics = app_state.peek().metrics.clone();
                let should_log = !metrics.waiting_loading_logged;
                if should_log {
                    telemetry::download_dispatch_waiting_loading(format.log_name());
                }
                let next =
                    download_effects::metrics_for_waiting_loading_phase(&metrics, should_log);
                if next != metrics {
                    app_state.with_mut(|state| state.metrics = next);
                }
            }
            DispatchPhase::WaitingForQuery { format } => {
                // Loading complete, waiting for query — log once via guard.
                let metrics = app_state.peek().metrics.clone();
                let should_log = !metrics.waiting_query_logged;
                if should_log {
                    telemetry::download_dispatch_waiting_query(format.log_name());
                }
                let next = download_effects::metrics_for_waiting_query_phase(&metrics, should_log);
                if next != metrics {
                    app_state.with_mut(|state| state.metrics = next);
                }
            }
            DispatchPhase::Ready {
                query,
                filename,
                format,
                #[cfg(target_arch = "wasm32")]
                criteria,
            } => {
                // All preconditions met — clear pending download and start dispatch.
                let has_pending_format = {
                    let state = app_state.read();
                    state.download.pending_format.is_some()
                };
                if has_pending_format {
                    app_state.with_mut(|state| {
                        state.download.pending_format = None;
                    });
                }
                dispatch_explore_action(explore, ExploreAction::DownloadDispatchStarted);
                telemetry::download_startup_dispatch_query_check(
                    format.log_name(),
                    query.contains("SERVICE"),
                    query.contains("SELECT"),
                    query.len(),
                );
                spawn(async move {
                    telemetry::download_dispatch_started(format.log_name());
                    if let Err(err) = execute_download(
                        format,
                        #[cfg(target_arch = "wasm32")]
                        criteria,
                        query,
                        filename,
                    )
                    .await
                    {
                        telemetry::download_dispatch_error(format.log_name(), &err);
                    }
                    dispatch_explore_action(explore, ExploreAction::DownloadDispatchFinished);
                });
            }
        }
    });
}
