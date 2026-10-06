// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

// Doc comments mix backticked SPARQL terms, Wikidata QIDs/P-IDs, and
// acronyms (QLever, WDQS) that are accurate as-is and read better without
// backticks or re-wording.
#![allow(clippy::doc_markdown)]

use crate::i18n::{
    curation_note_dependencies_pending, curation_note_existing_complete,
    curation_note_existing_updates, curation_note_new_compound, curation_pending_reference,
    curation_pending_taxon,
};
use crate::sparql::wdqs_download_query;
use crate::sparql::{FetchError, QLEVER_WIKIDATA, ResponseFormat};
#[cfg(not(target_arch = "wasm32"))]
use futures::future::BoxFuture;
#[cfg(target_arch = "wasm32")]
use futures::future::LocalBoxFuture;
pub(super) use lotus_curation::{
    CURATION_SPARQL_PREFIXES, CurationError, CurationInputRow, CurationResultRow, CurationStatus,
    DependencyResolution, MassResolution, WD_CHEMICAL_COMPOUND_QID, WD_OCCURS_IN_TAXON_PROP,
    WD_STEREOISOMER_GROUP_QID, WD_TAXON_QID, WD_TYPE_CHEMICAL_ENTITY_QID, WikidataCompound,
};

mod chemical;
mod enrichment;
mod helpers;
mod http_client;
mod occurrence;
// `unreachable_pub`-style narrowing requires `pub(crate)` here (used by
// `crate::curation`); the nursery `redundant_pub_crate` suggestion (`pub`)
// would widen it.
#[allow(clippy::redundant_pub_crate)]
pub(crate) mod occurrence_cache;
mod reference_metadata;
pub mod wikidata;

use chemical::{convert_smiles, has_undefined_stereo, resolve_exact_mass};
use helpers::{
    QS_REF_INFERRED_FROM_SMILES, escape_qs_string, has_isomeric_smiles, normalize_doi,
    qs_canonical_smiles_statement, qs_inchi_statement, qs_inchikey_statement,
    qs_isomeric_smiles_statement, qs_statement_with_refs,
};
use reference_metadata::fetch_reference_quickstatements;
use wikidata::normalize_taxon_lookup;

pub mod inputs;
pub mod pipeline;
pub mod prefetch;
pub mod quickstatements;

#[cfg(test)]
// `unreachable_pub`-style narrowing requires `pub(crate)` for this test-only
// re-export; the nursery `redundant_pub_crate` suggestion (`pub`) would widen it.
#[allow(clippy::redundant_pub_crate)]
pub(crate) use chemical::extract_exact_mass_from_json;
pub use enrichment::curate_single_row;
#[cfg(test)]
pub use helpers::{extract_formula_from_inchi, normalize_formula_for_wikidata, qs_mass_statement};
pub use prefetch::prefetch_knowledge;

#[cfg(not(target_arch = "wasm32"))]
type SparqlExecution<'a> = BoxFuture<'a, Result<String, FetchError>>;
#[cfg(target_arch = "wasm32")]
type SparqlExecution<'a> = LocalBoxFuture<'a, Result<String, FetchError>>;

/// Execute a SPARQL query against QLever, falling back to WDQS when QLever is
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
