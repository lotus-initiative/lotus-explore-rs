// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::features::explore::search_state::ExploreState;
use lotus_query::ExportFormat as DownloadFormat;
use lotus_search::SearchCriteria;
use std::sync::Arc;

/// Narrow view of download readiness state to avoid repeating complex queries.
// Cannot derive `Eq`: `Ready` holds `Arc<SearchCriteria>`, which has `f64` bounds.
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
        /// Criteria snapshot for the in-browser export, which embeds metadata a
        /// desktop write does not. Held on both targets so the variant's shape does
        /// not change with the build; only `execute_download` reads it.
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
        criteria: Arc::new(explore.ui.executed_criteria.clone()),
        query,
        filename,
        format,
    }
}
