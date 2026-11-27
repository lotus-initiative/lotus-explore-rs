// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Fetch the twelve Wikidata SPARQL tables required to build the `PubChem` trees.

use lotus_query::{
    COMPOUND_CXSMILES_QUERY, COMPOUND_INCHIKEY_TAXON_QUERY, COMPOUND_LABEL_QUERY,
    COMPOUND_PARENT_QUERY, COMPOUND_SMARTS_QUERY, COMPOUND_SMILES_CAN_ALL_QUERY,
    COMPOUND_SMILES_ISO_ALL_QUERY, REFERENCE_DOI_COMPOUND_TAXON_QUERY,
    REFERENCE_PMID_COMPOUND_TAXON_QUERY, TAXON_NAME_QUERY, TAXON_NCBI_QUERY, TAXON_PARENT_QUERY,
};
use lotus_search::{Endpoint, Http};

use crate::pipeline::{ExportError, Row, fetch_csv_rows};

/// All 12 tables fetched from Wikidata, in Python's `LOTUSData` order.
#[derive(Debug, Clone, Default)]
pub(super) struct PubChemData {
    pub(super) compound_taxon: Vec<Row>,
    pub(super) taxon_ncbi: Vec<Row>,
    pub(super) taxon_name: Vec<Row>,
    pub(super) taxon_parent: Vec<Row>,
    pub(super) compound_smiles_can: Vec<Row>,
    pub(super) compound_smiles_iso: Vec<Row>,
    pub(super) compound_smarts: Vec<Row>,
    pub(super) compound_cxsmiles: Vec<Row>,
    pub(super) compound_label: Vec<Row>,
    pub(super) compound_parent: Vec<Row>,
    pub(super) reference_doi: Vec<Row>,
    pub(super) reference_pmid: Vec<Row>,
}

/// The number of sequential HTTP fetches [`run_pubchem_tree`](super::run_pubchem_tree) performs
/// (12 SPARQL tables plus the `NPClassifier` cache CSV).
pub const PUBCHEM_TREE_QUERY_COUNT: u64 = 13;

/// Fetch all 12 Wikidata tables through the injected transport.
pub(super) async fn fetch_all_pubchem_data<H: Http>(
    http: &H,
    endpoint: &Endpoint,
    on_progress: &mut impl FnMut(&'static str),
) -> Result<PubChemData, ExportError> {
    async fn fetch<H: Http>(
        http: &H,
        endpoint: &Endpoint,
        query: &'static str,
        name: &'static str,
        on_progress: &mut impl FnMut(&'static str),
    ) -> Result<Vec<Row>, ExportError> {
        on_progress(name);
        fetch_csv_rows(http, endpoint, query).await
    }
    Ok(PubChemData {
        compound_taxon: fetch(
            http,
            endpoint,
            COMPOUND_INCHIKEY_TAXON_QUERY,
            "compound_taxon",
            on_progress,
        )
        .await?,
        taxon_ncbi: fetch(http, endpoint, TAXON_NCBI_QUERY, "taxon_ncbi", on_progress).await?,
        taxon_name: fetch(http, endpoint, TAXON_NAME_QUERY, "taxon_name", on_progress).await?,
        taxon_parent: fetch(
            http,
            endpoint,
            TAXON_PARENT_QUERY,
            "taxon_parent",
            on_progress,
        )
        .await?,
        compound_smiles_can: fetch(
            http,
            endpoint,
            COMPOUND_SMILES_CAN_ALL_QUERY,
            "compound_smiles_can",
            on_progress,
        )
        .await?,
        compound_smiles_iso: fetch(
            http,
            endpoint,
            COMPOUND_SMILES_ISO_ALL_QUERY,
            "compound_smiles_iso",
            on_progress,
        )
        .await?,
        compound_smarts: fetch(
            http,
            endpoint,
            COMPOUND_SMARTS_QUERY,
            "compound_smarts",
            on_progress,
        )
        .await?,
        compound_cxsmiles: fetch(
            http,
            endpoint,
            COMPOUND_CXSMILES_QUERY,
            "compound_cxsmiles",
            on_progress,
        )
        .await?,
        compound_label: fetch(
            http,
            endpoint,
            COMPOUND_LABEL_QUERY,
            "compound_label",
            on_progress,
        )
        .await?,
        compound_parent: fetch(
            http,
            endpoint,
            COMPOUND_PARENT_QUERY,
            "compound_parent",
            on_progress,
        )
        .await?,
        reference_doi: fetch(
            http,
            endpoint,
            REFERENCE_DOI_COMPOUND_TAXON_QUERY,
            "reference_doi",
            on_progress,
        )
        .await?,
        reference_pmid: fetch(
            http,
            endpoint,
            REFERENCE_PMID_COMPOUND_TAXON_QUERY,
            "reference_pmid",
            on_progress,
        )
        .await?,
    })
}
