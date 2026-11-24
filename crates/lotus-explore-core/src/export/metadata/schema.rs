// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Serializable Schema.org / `BioSchemas` DTOs for export metadata.

use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
pub(super) struct Organization<'a> {
    #[serde(rename = "@type")]
    pub(super) type_: &'a str,
    pub(super) name: &'a str,
    pub(super) url: &'a str,
}

#[derive(Serialize)]
pub(super) struct Creator<'a> {
    #[serde(rename = "@type")]
    pub(super) type_: &'a str,
    pub(super) name: &'a str,
    pub(super) version: &'a str,
    pub(super) url: &'a str,
}

#[derive(Serialize)]
pub(super) struct ScholarlyArticle<'a> {
    #[serde(rename = "@type")]
    pub(super) type_: &'a str,
    pub(super) name: &'a str,
    pub(super) identifier: &'a str,
    pub(super) url: &'a str,
}

#[derive(Serialize)]
pub(super) struct DataDownload {
    #[serde(rename = "@type")]
    pub(super) type_: &'static str,
    #[serde(rename = "encodingFormat")]
    pub(super) encoding_format: [&'static str; 2],
}

#[derive(Serialize)]
pub(super) struct ChemicalSearchService<'a> {
    pub(super) name: &'a str,
    pub(super) provider: &'a str,
    pub(super) endpoint: &'a str,
}

#[derive(Serialize)]
pub(super) struct SparqlEndpointInfo {
    pub(super) url: String,
    pub(super) name: String,
    pub(super) description: String,
}

#[derive(Serialize)]
pub(super) struct Provenance {
    pub(super) query_hash: HashInfo,
    pub(super) result_hash: HashInfo,
    pub(super) dataset_uri: String,
}

#[derive(Serialize)]
pub(super) struct HashInfo {
    pub(super) algorithm: &'static str,
    pub(super) value: String,
}

pub(super) const MEASURED_VARIABLES: &[&str] = &[
    "compound_name",
    "compound_smiles",
    "compound_inchikey",
    "compound_mass",
    "molecular_formula",
    "taxon_name",
    "reference_title",
    "reference_doi",
    "reference_date",
    "compound_qid",
    "taxon_qid",
    "reference_qid",
];

#[derive(Serialize)]
pub(super) struct DatasetMetadata {
    #[serde(rename = "@context")]
    pub(super) context: Value,
    #[serde(rename = "@type")]
    pub(super) type_: &'static str,
    #[serde(rename = "@id")]
    pub(super) id: String,
    #[serde(rename = "dct:conformsTo")]
    pub(super) conforms_to: &'static str,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) url: String,
    pub(super) identifier: Value,
    pub(super) keywords: Vec<&'static str>,
    pub(super) about: Value,
    pub(super) version: &'static str,
    #[serde(rename = "dateCreated")]
    pub(super) date_created: String,
    #[serde(rename = "datePublished")]
    pub(super) date_published: String,
    pub(super) license: &'static str,
    pub(super) creator: Creator<'static>,
    pub(super) provider: Vec<Organization<'static>>,
    pub(super) citation: Vec<ScholarlyArticle<'static>>,
    pub(super) distribution: Vec<DataDownload>,
    #[serde(rename = "includedInDataCatalog")]
    pub(super) included_in_data_catalog: Value,
    #[serde(rename = "measurementTechnique")]
    pub(super) measurement_technique: &'static str,
    #[serde(rename = "numberOfRecords", skip_serializing_if = "Option::is_none")]
    pub(super) number_of_records: Option<usize>,
    #[serde(rename = "variableMeasured")]
    pub(super) variable_measured: &'static [&'static str],
    pub(super) search_parameters: Value,
    #[serde(
        rename = "chemical_search_service",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) chemical_search_service: Option<ChemicalSearchService<'static>>,
    pub(super) sparql_endpoint: SparqlEndpointInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) sparql_query: Option<String>,
    pub(super) provenance: Provenance,
}
