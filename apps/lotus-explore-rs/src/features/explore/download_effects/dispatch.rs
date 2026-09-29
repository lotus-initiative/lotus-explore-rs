// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::features::explore::search_state::ExploreState;
use lotus_query::ExportFormat as DownloadFormat;
#[cfg(target_arch = "wasm32")]
use lotus_search::SearchCriteria;
use std::sync::Arc;

/// Narrow view of download readiness state to avoid repeating complex queries.
// Cannot derive `Eq`: on WASM the `Ready` variant holds `Arc<SearchCriteria>`,
// which contains `f64` bounds and is therefore not `Eq`. The native-only lint
// sees only the cfg-stripped shape and would otherwise demand it.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, Debug, PartialEq)]
pub enum DispatchPhase {
    /// No download pending — nothing to do.
    Inactive,
    /// Download pending, still waiting for results to load.
    WaitingForLoading { format: DownloadFormat },
    /// Loading complete, waiting for SPARQL query to materialize.
    WaitingForQuery { format: DownloadFormat },
    /// All preconditions met — ready to dispatch download.
    Ready {
        /// Criteria snapshot embedded in download metadata.
        /// Only materialized on WASM targets — desktop builds don't embed
        /// metadata in files so the clone is skipped entirely.
        #[cfg(target_arch = "wasm32")]
        criteria: Arc<SearchCriteria>,
        /// Query to pass to download executor.
        query: Arc<str>,
        /// Filename to use for downloaded file.
        filename: String,
        /// Download format (for telemetry).
        format: DownloadFormat,
    },
}

/// Determine the current dispatch phase based on download and result state.
#[must_use]
pub fn classify_dispatch_phase(
    pending_format: Option<DownloadFormat>,
    explore: &ExploreState,
) -> DispatchPhase {
    let Some(format) = pending_format else {
        return DispatchPhase::Inactive;
    };

    if explore.lifecycle.loading {
        return DispatchPhase::WaitingForLoading { format };
    }

    let Some(query) = explore.result.sparql_query.clone() else {
        return DispatchPhase::WaitingForQuery { format };
    };

    let filename =
        crate::export::generate_filename(&explore.ui.executed_criteria, format.extension());

    DispatchPhase::Ready {
        #[cfg(target_arch = "wasm32")]
        criteria: Arc::new(explore.ui.executed_criteria.clone()),
        query,
        filename,
        format,
    }
}
