// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Shared search outcome type for completed Explore executions.

use crate::api::SearchResponse;
use crate::features::explore::service::results_pipeline::ResultsPipelineOutcome;
use crate::features::explore::types::TaxonWarning;
use lotus_model::{CompoundEntry, DatasetStats};

/// The raw outcome from a completed search execution.
pub struct SearchOutcome {
    pub rows: Vec<CompoundEntry>,
    pub qid: Option<String>,
    /// Everything to tell the user about how this search went. Usually nothing;
    /// more than one entry when a taxon name both needed standardizing and was
    /// ambiguous, or when the endpoint had to change.
    pub warnings: Vec<TaxonWarning>,
    pub query: String,
    pub total_matches: Option<usize>,
    pub total_stats: Option<DatasetStats>,
    pub display_capped_rows: bool,
    pub endpoint: crate::export::SparqlEndpoint,
}

impl SearchOutcome {
    #[must_use]
    pub fn from_api_response(
        response: SearchResponse,
        display_limit: usize,
        include_counts: bool,
    ) -> Self {
        let display_capped_rows = if include_counts {
            response.total_matches > response.rows.len()
        } else {
            response.rows.len() >= display_limit
        };
        let rows = response
            .rows
            .into_iter()
            .map(CompoundEntry::from)
            .collect::<Vec<_>>();
        let warnings = response
            .warning
            .into_iter()
            .map(TaxonWarning::ApiMessage)
            .collect();

        Self {
            rows,
            qid: response.resolved_taxon_qid,
            warnings,
            query: response.query,
            total_matches: Some(response.total_matches),
            total_stats: Some(response.stats.into()),
            display_capped_rows,
            endpoint: crate::export::SparqlEndpoint::Qlever,
        }
    }

    #[must_use]
    pub fn from_results_pipeline(outcome: ResultsPipelineOutcome) -> Self {
        Self {
            rows: outcome.rows,
            qid: outcome.qid,
            warnings: outcome.warnings,
            query: outcome.query,
            total_matches: outcome.total_matches,
            total_stats: outcome.total_stats,
            display_capped_rows: outcome.display_capped_rows,
            endpoint: outcome.endpoint,
        }
    }
}
