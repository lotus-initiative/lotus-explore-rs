// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
// The search future is not `Send`: `on_phase` is a closure over a Dioxus
// `Signal`, which is a `RefCell`. See the full explanation in
// `features/explore/executor.rs`, which is the same cause reached from here.
#![expect(
    clippy::future_not_send,
    reason = "`on_phase` captures a Dioxus `Signal`, which is a `RefCell` and not `Sync`"
)]

use super::ResultsPipelineOutcome;
use crate::export::SparqlEndpoint;
use crate::features::explore::request::SearchRequest;
use crate::features::explore::search_metrics::SearchMetrics;
use crate::features::explore::service::{
    build_query::{ResolvedStructure, apply_server_filters, build_sparql_query},
    fetch_results::FetchResult,
    resolve_reference,
    resolve_structure::{self, StructureResolution},
    resolve_taxon::{self, TaxonResolution},
};
use crate::features::explore::types::{DomainError, QueryPhase};
use crate::repositories::{LotusRepository, is_wdqs_fallback_used};

pub(super) struct ResultsExecutionPlan {
    taxon_resolution: TaxonResolution,
    structure_resolution: StructureResolution,
    execution_query: String,
}

impl ResultsExecutionPlan {
    pub(super) fn execution_query(&self) -> &str {
        &self.execution_query
    }

    pub(super) fn into_download_only_outcome(self) -> ResultsPipelineOutcome {
        self.into_outcome(FetchResult::empty())
    }

    pub(super) fn into_interactive_outcome(
        self,
        fetch_result: FetchResult,
    ) -> ResultsPipelineOutcome {
        self.into_outcome(fetch_result)
    }

    /// The endpoint, warnings and display query are a function of the fallback state
    /// alone, so the two outcomes differ only in what was fetched: a download-only
    /// search fetched nothing, which is `FetchResult::empty`.
    fn into_outcome(self, fetch_result: FetchResult) -> ResultsPipelineOutcome {
        let endpoint = if is_wdqs_fallback_used() {
            SparqlEndpoint::Wdqs
        } else {
            SparqlEndpoint::Qlever
        };
        // A fallback used to replace the taxon's notices rather than add to them, so a
        // name that needed standardizing stopped being reported the moment the
        // endpoint had to change.
        let mut warnings = self.taxon_resolution.warnings;
        warnings.extend(self.structure_resolution.notices.clone());
        if is_wdqs_fallback_used() {
            warnings.push(crate::features::explore::types::LookupNotice::WdqsFallback);
        }
        let query = crate::repositories::get_wdqs_transformed_query()
            .unwrap_or_else(|| self.execution_query.clone());

        ResultsPipelineOutcome {
            set: fetch_result.set,
            qid: self.taxon_resolution.qid,
            warnings,
            query,
            display_capped_rows: fetch_result.display_capped_rows,
            endpoint,
        }
    }
}

pub(super) async fn build_execution_plan<R: LotusRepository>(
    request: &SearchRequest,
    normalized_smiles: &str,
    repo: &R,
    metrics: &mut SearchMetrics,
    on_phase: &impl Fn(QueryPhase),
) -> Result<ResultsExecutionPlan, DomainError> {
    let taxon = request.criteria().taxon.trim();
    if resolve_taxon::requires_remote_lookup(taxon) {
        on_phase(QueryPhase::ResolvingTaxon);
    }

    let taxon_resolution = resolve_taxon::resolve(taxon, repo, metrics).await?;

    // The structure field is resolved like the taxon field, for the same reason:
    // whatever is typed here should find the compound it names. It runs for every
    // input, a structure included, because two of the three modes need a QID —
    // which is why it happens before the query is built rather than inside it.
    let structure_resolution = if normalized_smiles.trim().is_empty() {
        StructureResolution {
            resolved: ResolvedStructure::unresolved(""),
            notices: Vec::new(),
        }
    } else {
        on_phase(QueryPhase::ResolvingStructure);
        resolve_structure::resolve(normalized_smiles, repo, metrics).await?
    };

    // The reference resolves like the taxon, then is written back into a copy of the
    // criteria as the QID it resolved to. The seed binds `?r`, so it must be the
    // item: a DOI is not a QID and `VALUES ?r { wd:10.1002/… }` matches nothing.
    //
    // A copy, because the form renders from the criteria and the field must keep
    // showing the DOI that was typed.
    let reference = request.criteria().reference.trim();
    let reference_resolution = if resolve_reference::requires_remote_lookup(reference) {
        on_phase(QueryPhase::ResolvingReference);
        resolve_reference::resolve(reference, repo, metrics).await?
    } else {
        resolve_reference::ReferenceResolution {
            qid: None,
            notices: Vec::new(),
        }
    };
    let mut criteria = request.criteria().clone();
    criteria
        .reference
        .clone_from(&reference_resolution.qid.unwrap_or_default());

    let sparql_query = build_sparql_query(
        &structure_resolution.resolved,
        &criteria,
        taxon_resolution.qid.as_deref(),
    );
    let execution_query = apply_server_filters(&sparql_query, &criteria);

    Ok(ResultsExecutionPlan {
        taxon_resolution,
        structure_resolution,
        execution_query,
    })
}
