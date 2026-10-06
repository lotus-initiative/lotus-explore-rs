// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Pure model helpers for results-toolbar download actions.

use crate::export;
use crate::i18n::TextKey;
use lotus_query::ExportFormat as DownloadFormat;
use lotus_search::SearchCriteria;

const QLEVER_UI: &str = "https://qlever.dev/wikidata";
const WDQS_UI: &str = "https://query.wikidata.org";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(super) enum SparqlEndpointUI {
    /// `QLever` SPARQL endpoint (default)
    #[default]
    Qlever,
    /// Wikidata Query Service endpoint (fallback)
    Wdqs,
}

impl From<export::SparqlEndpoint> for SparqlEndpointUI {
    fn from(value: export::SparqlEndpoint) -> Self {
        match value {
            export::SparqlEndpoint::Qlever => Self::Qlever,
            export::SparqlEndpoint::Wdqs => Self::Wdqs,
        }
    }
}

impl std::fmt::Display for SparqlEndpointUI {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Qlever => write!(f, "QLever SPARQL"),
            Self::Wdqs => write!(f, "Wikidata Query Service"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DownloadQuerySpec {
    pub(super) format: DownloadFormat,
    pub(super) status_key: TextKey,
    pub(super) title_key: TextKey,
    pub(super) label_key: TextKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DownloadMetadataSpec {
    pub(super) title_key: TextKey,
    pub(super) label_key: TextKey,
}

pub(super) const DOWNLOAD_QUERY_CSV_SPEC: DownloadQuerySpec = DownloadQuerySpec {
    format: DownloadFormat::Csv,
    status_key: TextKey::StartingCsvDownload,
    title_key: TextKey::DownloadCsvTitle,
    label_key: TextKey::DownloadCsvLabel,
};

pub(super) const DOWNLOAD_QUERY_JSON_SPEC: DownloadQuerySpec = DownloadQuerySpec {
    format: DownloadFormat::Json,
    status_key: TextKey::PreparingJsonDownload,
    title_key: TextKey::DownloadJsonTitle,
    label_key: TextKey::DownloadJsonLabel,
};

pub(super) const DOWNLOAD_QUERY_RDF_SPEC: DownloadQuerySpec = DownloadQuerySpec {
    format: DownloadFormat::Rdf,
    status_key: TextKey::PreparingRdfDownload,
    title_key: TextKey::DownloadRdfTitle,
    label_key: TextKey::DownloadRdfLabel,
};

pub(super) const DOWNLOAD_METADATA_SPEC: DownloadMetadataSpec = DownloadMetadataSpec {
    title_key: TextKey::DownloadMetadataTitle,
    label_key: TextKey::DownloadMetadataLabel,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DownloadToolbarModel {
    pub(super) export_available: bool,
    pub(super) csv_filename: String,
    pub(super) json_filename: String,
    pub(super) rdf_filename: String,
    pub(super) metadata_filename: String,
    pub(super) sparql_endpoint_ui: SparqlEndpointUI,
    pub(super) ui_url: Option<String>,
}

#[must_use]
// Only the tests below read this, so it is not compiled into a binary that
// has no tests to run.
#[cfg(test)]
pub(super) fn build_download_toolbar_model(
    criteria: &SearchCriteria,
    sparql_query: Option<&str>,
    metadata_json: Option<&str>,
    query_hash: Option<&str>,
    result_hash: Option<&str>,
) -> DownloadToolbarModel {
    build_download_toolbar_model_with_endpoint(
        criteria,
        sparql_query,
        metadata_json,
        query_hash,
        result_hash,
        SparqlEndpointUI::default(),
    )
}

#[must_use]
pub(super) fn build_download_toolbar_model_with_endpoint(
    criteria: &SearchCriteria,
    sparql_query: Option<&str>,
    metadata_json: Option<&str>,
    query_hash: Option<&str>,
    result_hash: Option<&str>,
    endpoint: SparqlEndpointUI,
) -> DownloadToolbarModel {
    DownloadToolbarModel {
        export_available: sparql_query.is_some() || metadata_json.is_some(),
        csv_filename: export::generate_filename(criteria, DownloadFormat::Csv.extension()),
        json_filename: export::generate_filename(criteria, DownloadFormat::Json.extension()),
        rdf_filename: export::generate_filename(criteria, DownloadFormat::Rdf.extension()),
        metadata_filename: build_metadata_filename(criteria, query_hash, result_hash),
        sparql_endpoint_ui: endpoint,
        ui_url: build_sparql_ui_url(sparql_query, endpoint),
    }
}

#[must_use]
fn build_metadata_filename(
    criteria: &SearchCriteria,
    query_hash: Option<&str>,
    result_hash: Option<&str>,
) -> String {
    match (query_hash, result_hash) {
        (Some(query_hash), Some(result_hash)) => {
            format!("{query_hash}_{result_hash}_metadata.json")
        }
        _ => export::generate_filename(criteria, "metadata.json"),
    }
}

#[must_use]
fn build_sparql_ui_url(sparql_query: Option<&str>, endpoint: SparqlEndpointUI) -> Option<String> {
    let base_url = match endpoint {
        SparqlEndpointUI::Qlever => QLEVER_UI,
        SparqlEndpointUI::Wdqs => WDQS_UI,
    };
    sparql_query.map(|query| format!("{base_url}?query={}", urlencoding::encode(query)))
}

#[cfg(test)]
#[path = "download_model/tests.rs"]
mod tests;
