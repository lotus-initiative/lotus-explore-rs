// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::i18n::{
    curation_note_dependencies_pending, curation_note_existing_complete,
    curation_note_existing_updates, curation_note_missing_doi, curation_note_new_compound,
    curation_note_revised, curation_pending_reference, curation_pending_taxon,
};
use crate::sparql::wdqs_download_query;
#[cfg(not(target_arch = "wasm32"))]
use futures::future::BoxFuture;
#[cfg(target_arch = "wasm32")]
use futures::future::LocalBoxFuture;
use lotus_model::QLEVER_WIKIDATA;
use lotus_search::{FetchError, ResponseFormat};

#[cfg(test)]
pub mod chemical;
#[cfg(not(test))]
mod chemical;
pub mod enrichment;
mod http_client;
mod occurrence;
pub mod occurrence_cache;
mod reference_metadata;
pub mod wikidata;

use chemical::{convert_smiles, has_undefined_stereo, resolve_exact_mass};
use reference_metadata::fetch_reference_quickstatements;

pub mod inputs;
pub mod pipeline;
pub mod prefetch;
pub mod quickstatements;

#[cfg(not(target_arch = "wasm32"))]
type SparqlExecution<'a> = BoxFuture<'a, Result<String, FetchError>>;
#[cfg(target_arch = "wasm32")]
type SparqlExecution<'a> = LocalBoxFuture<'a, Result<String, FetchError>>;

/// Execute a SPARQL query against `QLever`, falling back to `WDQS` when `QLever` is
/// unavailable.
pub async fn execute_sparql_with_wdqs_fallback(
    query: &str,
    format: ResponseFormat,
) -> Result<String, FetchError> {
    execute_sparql_with_wdqs_fallback_with(query, format, |query, endpoint, format| {
        Box::pin(crate::sparql::execute_sparql_format_at(
            query, endpoint, format,
        ))
    })
    .await
}

async fn execute_sparql_with_wdqs_fallback_with<F>(
    query: &str,
    format: ResponseFormat,
    mut execute: F,
) -> Result<String, FetchError>
where
    F: Send + for<'a> FnMut(&'a str, &'static str, ResponseFormat) -> SparqlExecution<'a>,
{
    let result = execute(query, QLEVER_WIKIDATA, format).await;

    match result {
        Ok(response) => Ok(response),
        Err(error) if should_fallback_to_wdqs(&error) => {
            log::warn!("event=curation_sparql phase=fallback reason=qlever_unavailable");
            let (endpoint, fallback_query) = wdqs_download_query(query);
            execute(&fallback_query, endpoint, format).await
        }
        Err(error) => Err(error),
    }
}

fn should_fallback_to_wdqs(error: &FetchError) -> bool {
    matches!(
        error,
        FetchError::Http { status: 502, .. } | FetchError::Network(_)
    )
}

#[cfg(test)]
mod tests;
