// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `download_model`, in their own file.

#![allow(clippy::expect_used)]

use super::{
    DOWNLOAD_QUERY_CSV_SPEC, DOWNLOAD_QUERY_JSON_SPEC, DOWNLOAD_QUERY_RDF_SPEC, SparqlEndpointUI,
    build_download_toolbar_model, build_download_toolbar_model_with_endpoint,
};
use lotus_query::ExportFormat as DownloadFormat;
use lotus_search::SearchCriteria;

#[test]
fn download_specs_keep_expected_formats() {
    assert_eq!(DOWNLOAD_QUERY_CSV_SPEC.format, DownloadFormat::Csv);
    assert_eq!(DOWNLOAD_QUERY_JSON_SPEC.format, DownloadFormat::Json);
    assert_eq!(DOWNLOAD_QUERY_RDF_SPEC.format, DownloadFormat::Rdf);
}

/// The name the toolbar shows is the name the download gets.
///
/// These three used to be spelled as literals next to each other, which is how
/// `rdf_filename` outlived the rename to `ttl`: `extension()` changed, and a
/// literal three lines away did not. The reader was shown one name and handed a
/// file under another.
///
/// Derived from the format rather than written out, so a format that is renamed
/// renames the toolbar with it and there is nothing left here to keep in step.
#[test]
fn the_toolbar_filenames_follow_the_formats_own_extension() {
    let criteria = SearchCriteria::up_to_year(crate::clock::current_year());
    let model = build_download_toolbar_model(
        &criteria,
        Some("SELECT * WHERE { ?s ?p ?o }"),
        None,
        None,
        None,
    );

    for (name, format) in [
        (&model.csv_filename, DownloadFormat::Csv),
        (&model.json_filename, DownloadFormat::Json),
        (&model.rdf_filename, DownloadFormat::Rdf),
    ] {
        assert!(
            name.ends_with(&format!(".{}", format.extension())),
            "{name} does not carry {}'s extension",
            format.log_name()
        );
    }

    assert!(
        model
            .rdf_filename
            .ends_with(&format!(".{}", DownloadFormat::Rdf.extension())),
        "Turtle is named for what it is, and this is the assertion that would have \
         caught the toolbar still saying rdf after the rename: {}",
        model.rdf_filename
    );
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

    let model = build_download_toolbar_model(&criteria, None, Some("{}"), Some("query123"), None);

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
