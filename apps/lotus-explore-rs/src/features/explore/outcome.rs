// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Shared search outcome type for completed Explore executions.

use crate::api::SearchResponse;
use crate::features::explore::service::results_pipeline::ResultsPipelineOutcome;
use crate::features::explore::types::LookupNotice;
use lotus_model::{ColumnarResultSet, CompoundEntry};
use std::sync::Arc;

/// The raw outcome from a completed search execution.
pub struct SearchOutcome {
    pub set: std::sync::Arc<ColumnarResultSet>,
    pub qid: Option<String>,
    /// Everything to tell the user about how this search went. Usually nothing;
    /// more than one entry when a taxon name both needed standardizing and was
    /// ambiguous, or when the endpoint had to change.
    pub warnings: Vec<LookupNotice>,
    pub query: String,
    pub display_capped_rows: bool,
    pub endpoint: crate::export::SparqlEndpoint,
}

impl SearchOutcome {
    /// Fold a REST response into the same shape the SPARQL path produces.
    ///
    /// Only called when the response holds the whole result set; see
    /// `api_pipeline` for why a partial page is declined rather than counted.
    /// Nothing is truncated, so `display_capped_rows` is always false -- the set
    /// cannot be hiding rows that were never fetched.
    #[must_use]
    pub fn from_api_response(response: SearchResponse) -> Self {
        let display_capped_rows = false;
        // The API fast path still answers in rows. Folding them into a set here
        // rather than at the reducer keeps one representation downstream, so the
        // table cannot behave differently depending on which endpoint served it.
        let set = Arc::new(ColumnarResultSet::from_entries(
            &response
                .rows
                .into_iter()
                .map(CompoundEntry::from)
                .collect::<Vec<_>>(),
        ));
        let warnings = response
            .warning
            .into_iter()
            .map(LookupNotice::ApiMessage)
            .collect();

        Self {
            set,
            qid: response.resolved_taxon_qid,
            warnings,
            query: response.query,
            display_capped_rows,
            endpoint: crate::export::SparqlEndpoint::Qlever,
        }
    }

    #[must_use]
    pub fn from_results_pipeline(outcome: ResultsPipelineOutcome) -> Self {
        Self {
            set: outcome.set,
            qid: outcome.qid,
            warnings: outcome.warnings,
            query: outcome.query,
            display_capped_rows: outcome.display_capped_rows,
            endpoint: outcome.endpoint,
        }
    }
}
