// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `export`, in their own file.

use super::*;

#[test]
fn parse_known_formats() {
    assert_eq!(ExportFormat::parse("csv"), Some(ExportFormat::Csv));
    assert_eq!(ExportFormat::parse("json"), Some(ExportFormat::Json));
    assert_eq!(ExportFormat::parse("ndjson"), Some(ExportFormat::Json));
    assert_eq!(ExportFormat::parse("rdf"), Some(ExportFormat::Rdf));
    assert_eq!(ExportFormat::parse(" JSON "), Some(ExportFormat::Json));
    assert_eq!(ExportFormat::parse("RDF"), Some(ExportFormat::Rdf));
}

#[test]
fn parse_rejects_unknown_and_empty() {
    assert_eq!(ExportFormat::parse(""), None);
    assert_eq!(ExportFormat::parse("   "), None);
    assert_eq!(ExportFormat::parse("xyz"), None);
    assert_eq!(ExportFormat::parse("xml"), None);
}

#[test]
fn a_mime_type_is_not_a_format_name() {
    // The response is `text/turtle`, but that is a content type rather
    // than a format, and treating it as one would make the accepted set
    // larger than the documented one.
    assert_eq!(ExportFormat::parse("text/turtle"), None);
    assert_eq!(ExportFormat::parse("ttl"), None);
}

#[test]
fn parse_round_trips_through_extension() {
    for fmt in [ExportFormat::Csv, ExportFormat::Json, ExportFormat::Rdf] {
        assert_eq!(ExportFormat::parse(fmt.extension()), Some(fmt));
    }
}

#[test]
fn qlever_actions_are_stable() {
    // These are QLever's own parameter names. They are part of the URL it
    // is asked for, so changing one silently breaks every bookmarked
    // export link.
    assert_eq!(ExportFormat::Csv.qlever_action(), "csv_export");
    assert_eq!(ExportFormat::Json.qlever_action(), "qlever_json_export");
    assert_eq!(ExportFormat::Rdf.qlever_action(), "turtle_export");
}

#[test]
fn prepared_query_wraps_rdf_in_construct() {
    let select = "PREFIX wd: <http://www.wikidata.org/entity/>\nSELECT ?s WHERE { ?s ?p ?o }";

    assert_eq!(ExportFormat::Csv.prepared_query(select), select);
    assert_eq!(ExportFormat::Json.prepared_query(select), select);

    let rdf = ExportFormat::Rdf.prepared_query(select);
    assert!(rdf.contains("CONSTRUCT"), "{rdf}");
    assert!(rdf.contains("WHERE"), "{rdf}");
}

#[test]
fn content_types_match_the_format() {
    assert!(ExportFormat::Csv.content_type().starts_with("text/csv"));
    assert!(
        ExportFormat::Json
            .content_type()
            .starts_with("application/")
    );
    assert!(ExportFormat::Rdf.content_type().starts_with("text/turtle"));
}

#[test]
fn sanitize_removes_path_traversal() {
    let name = sanitize_download_filename("../../etc/passwd");
    assert!(!name.contains('/'), "{name}");
    assert!(!name.contains('\\'), "{name}");
}

#[test]
fn sanitize_keeps_ordinary_names_unchanged() {
    assert_eq!(
        sanitize_download_filename("lotus_results.csv"),
        "lotus_results.csv"
    );
    assert_eq!(
        sanitize_download_filename("natural-products.json"),
        "natural-products.json"
    );
}

#[test]
fn sanitize_drops_control_characters() {
    assert_eq!(sanitize_download_filename("a\u{0}b\rc.csv"), "abc.csv");
}

#[test]
fn sanitize_cannot_produce_a_path() {
    // Dots and separators are stripped from both ends, so a name made only
    // of them cannot survive as "." or "..", the two names a download path
    // can be redirected through. An empty result is fine: the browser picks
    // its own name then.
    for hostile in ["..", "../..", ".", "./.", "..."] {
        let name = sanitize_download_filename(hostile);
        assert!(!name.contains('/'), "{hostile:?} -> {name:?}");
        assert!(!name.contains('\\'), "{hostile:?} -> {name:?}");
        assert!(name != "." && name != "..", "{hostile:?} -> {name:?}");
    }
}
