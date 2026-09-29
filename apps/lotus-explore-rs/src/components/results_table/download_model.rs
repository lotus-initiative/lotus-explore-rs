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
        csv_filename: export::generate_filename(criteria, "csv"),
        json_filename: export::generate_filename(criteria, "json"),
        rdf_filename: export::generate_filename(criteria, "rdf"),
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
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        DOWNLOAD_QUERY_CSV_SPEC, DOWNLOAD_QUERY_JSON_SPEC, DOWNLOAD_QUERY_RDF_SPEC,
        SparqlEndpointUI, build_download_toolbar_model, build_download_toolbar_model_with_endpoint,
    };
    use lotus_query::ExportFormat as DownloadFormat;
    use lotus_search::SearchCriteria;

    #[test]
    fn download_specs_keep_expected_formats() {
        assert_eq!(DOWNLOAD_QUERY_CSV_SPEC.format, DownloadFormat::Csv);
        assert_eq!(DOWNLOAD_QUERY_JSON_SPEC.format, DownloadFormat::Json);
        assert_eq!(DOWNLOAD_QUERY_RDF_SPEC.format, DownloadFormat::Rdf);
    }

    #[test]
    fn toolbar_model_uses_hashes_for_metadata_filename_when_both_are_present() {
        let criteria = SearchCriteria::up_to_year(crate::clock::current_year());

        let model = build_download_toolbar_model(
            &criteria,
            Some("SELECT * WHERE { ?s ?p ?o }"),
            Some("{\"meta\":true}"),
            Some("query123"),
            Some("result456"),
        );

        assert!(model.export_available);
        assert_eq!(model.metadata_filename, "query123_result456_metadata.json");
    }

    #[test]
    fn toolbar_model_falls_back_to_generated_metadata_filename_without_both_hashes() {
        let criteria = SearchCriteria::up_to_year(crate::clock::current_year());

        let model =
            build_download_toolbar_model(&criteria, None, Some("{}"), Some("query123"), None);

        assert!(model.export_available);
        assert!(model.metadata_filename.ends_with("metadata.json"));
        assert_ne!(model.metadata_filename, "query123_metadata.json");
    }

    #[test]
    fn toolbar_model_leaves_exports_hidden_when_no_query_or_metadata_exist() {
        let criteria = SearchCriteria::up_to_year(crate::clock::current_year());

        let model = build_download_toolbar_model(&criteria, None, None, None, None);

        assert!(!model.export_available);
        assert!(model.ui_url.is_none());
    }

    #[test]
    fn toolbar_model_encodes_query_for_qlever_ui_link() {
        let criteria = SearchCriteria::up_to_year(crate::clock::current_year());
        let query = "SELECT * WHERE { ?compound wdt:P31 \"natural product\" }";

        let model = build_download_toolbar_model(&criteria, Some(query), None, None, None);

        let url = model.ui_url.expect("query link should be present");
        let encoded = urlencoding::encode(query);
        assert!(url.starts_with("https://qlever.dev/wikidata?query="));
        assert!(url.contains(encoded.as_ref()));
    }

    #[test]
    fn toolbar_model_encodes_query_for_wdqs_ui_link_when_endpoint_is_wdqs() {
        let criteria = SearchCriteria::up_to_year(crate::clock::current_year());
        let query = "SELECT * WHERE { ?compound wdt:P31 \"natural product\" }";

        let model = build_download_toolbar_model_with_endpoint(
            &criteria,
            Some(query),
            None,
            None,
            None,
            SparqlEndpointUI::Wdqs,
        );

        let url = model.ui_url.expect("query link should be present");
        let encoded = urlencoding::encode(query);
        assert!(url.starts_with("https://query.wikidata.org?query="));
        assert!(url.contains(encoded.as_ref()));
    }

    #[test]
    fn toolbar_model_shows_correct_endpoint_name() {
        let criteria = SearchCriteria::up_to_year(crate::clock::current_year());

        let qlever_model = build_download_toolbar_model(
            &criteria,
            Some("SELECT ?s WHERE { ?s ?p ?o }"),
            None,
            None,
            None,
        );
        assert_eq!(qlever_model.sparql_endpoint_ui.to_string(), "QLever SPARQL");

        let wdqs_model = build_download_toolbar_model_with_endpoint(
            &criteria,
            Some("SELECT ?s WHERE { ?s ?p ?o }"),
            None,
            None,
            None,
            SparqlEndpointUI::Wdqs,
        );
        assert_eq!(
            wdqs_model.sparql_endpoint_ui.to_string(),
            "Wikidata Query Service"
        );
    }
}
