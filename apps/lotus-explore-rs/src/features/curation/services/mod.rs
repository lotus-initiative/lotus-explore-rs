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
pub mod quickstatements;

#[cfg(test)]
// `unreachable_pub`-style narrowing requires `pub(crate)` for this test-only
// re-export; the nursery `redundant_pub_crate` suggestion (`pub`) would widen it.
#[allow(clippy::redundant_pub_crate)]
pub(crate) use chemical::extract_exact_mass_from_json;
pub use enrichment::curate_single_row;
#[cfg(test)]
pub use helpers::{extract_formula_from_inchi, normalize_formula_for_wikidata, qs_mass_statement};

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
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::sparql::{QLEVER_WIKIDATA, WDQS_SCHOLARLY, WDQS_WIKIDATA};
    use futures::executor::block_on;
    use std::sync::{Arc, Mutex};

    fn run_with_mock(
        query: &str,
        first_result: Result<String, FetchError>,
    ) -> (Vec<(String, &'static str)>, Result<String, FetchError>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let calls_for_executor = Arc::clone(&calls);
        let result = block_on(execute_sparql_with_wdqs_fallback_with(
            query,
            ResponseFormat::SparqlJson,
            move |query, endpoint, _| {
                let call_index = calls_for_executor.lock().expect("calls lock").len();
                calls_for_executor
                    .lock()
                    .expect("calls lock")
                    .push((query.to_owned(), endpoint));
                let response = if call_index == 0 {
                    first_result.clone()
                } else {
                    Ok("ok".to_owned())
                };
                Box::pin(async move { response })
            },
        ));
        let recorded = calls.lock().expect("calls lock").clone();
        (recorded, result)
    }

    #[test]
    fn qlever_success_never_uses_wdqs() {
        let query = "SELECT ?ref WHERE { ?ref wdt:P356 \"10.1/x\" }";
        let (calls, result) = run_with_mock(query, Ok("qlever".to_owned()));

        assert!(result.is_ok());
        assert_eq!(calls, vec![(query.to_owned(), QLEVER_WIKIDATA)]);
    }

    #[test]
    fn qlever_network_failure_falls_back() {
        let query = "SELECT ?item WHERE { ?item wdt:P31 wd:Q16521 }";
        let (calls, result) = run_with_mock(
            query,
            Err(FetchError::Network("connection failed".to_owned())),
        );

        assert!(result.is_ok());
        assert_eq!(calls.len(), 2);
        assert_eq!(calls.first().expect("QLever call").1, QLEVER_WIKIDATA);
        assert_eq!(calls.get(1).expect("fallback call").1, WDQS_WIKIDATA);
    }

    #[test]
    fn qlever_502_falls_back_by_query_kind() {
        let cases = [
            (
                "SELECT ?item WHERE { ?item wdt:P31 wd:Q16521 }",
                WDQS_WIKIDATA,
            ),
            (
                "SELECT ?ref WHERE { ?ref wdt:P356 \"10.1/x\" }",
                WDQS_SCHOLARLY,
            ),
        ];

        for (query, expected_endpoint) in cases {
            let gateway = FetchError::Http {
                status: 502,
                message: "bad gateway".to_owned(),
            };
            let (calls, result) = run_with_mock(query, Err(gateway));

            assert!(result.is_ok());
            assert_eq!(calls.len(), 2);
            let first = calls.first().expect("QLever call");
            let second = calls.get(1).expect("fallback call");
            assert_eq!(first.1, QLEVER_WIKIDATA);
            assert_eq!(second.1, expected_endpoint);
            assert_eq!(first.0, query);
            assert_eq!(second.0, query);
        }
    }
}
