// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The three archive formats, and how each one changes a query.
//!
//! A format is a decision about *how* the answer is delivered, and changing a
//! query to suit a format is query construction, so it lives next to the query
//! builders rather than in whichever layer happens to be offering a download.
//!
//! [`ResponseFormat`] is deliberately absent: mapping an export format onto a
//! content-negotiated request is the transport's business, and `lotus-search`
//! does it with [`From`].

use crate::query::construct_from_select;

/// The three archive formats supported by the LOTUS/QLever export pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExportFormat {
    /// Comma-separated values — the compact, lossless default for bulk export.
    Csv,
    /// JSON in the [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/),
    /// also used for `ndjson` (one JSON object per line).
    Json,
    /// RDF/Turtle triples via a `CONSTRUCT` query, suitable for ingestion
    /// into triple stores.
    Rdf,
}

impl ExportFormat {
    /// Parse from a CLI argument, URL fragment or header ("csv", "json",
    /// "ndjson", "rdf"). Returns `None` for anything else.
    ///
    /// The three names are the whole documented set. Accepting `ttl` or
    /// `turtle` would be friendlier, but the served content type is
    /// `text/turtle` while the requested name is `rdf`, and quietly widening
    /// what a URL accepts means a link that works today 404s after a rename.
    /// A format that is not in the documentation should be added to the
    /// documentation first.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "csv" => Some(Self::Csv),
            "json" | "ndjson" => Some(Self::Json),
            "rdf" => Some(Self::Rdf),
            _ => None,
        }
    }

    /// File extension without a leading dot.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
            Self::Rdf => "rdf",
        }
    }

    /// `QLever`'s `action=` parameter value for this format.
    #[must_use]
    pub const fn qlever_action(self) -> &'static str {
        match self {
            Self::Csv => "csv_export",
            Self::Json => "qlever_json_export",
            Self::Rdf => "turtle_export",
        }
    }

    /// The query to actually send, which for RDF is not the query you wrote.
    ///
    /// `QLever` will not turn a `SELECT` into triples, so an RDF export has to
    /// be wrapped in a `CONSTRUCT` that names the columns it wants as
    /// predicates. CSV and JSON are passed through untouched.
    #[must_use]
    pub fn prepared_query(self, query: &str) -> String {
        match self {
            Self::Rdf => construct_from_select(query),
            _ => query.to_string(),
        }
    }

    /// The `Content-Type` a server should send back.
    #[must_use]
    pub const fn content_type(self) -> &'static str {
        match self {
            Self::Csv => "text/csv;charset=utf-8",
            Self::Json => "application/sparql-results+json;charset=utf-8",
            Self::Rdf => "text/turtle;charset=utf-8",
        }
    }

    /// Short name for a log line, a metric label or a filename.
    #[must_use]
    pub const fn log_name(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
            Self::Rdf => "rdf",
        }
    }
}

/// A filename that is safe to hand to a browser's download attribute.
///
/// Control characters are dropped, and path separators, quotes and newlines
/// become underscores, so a name built from a query string cannot walk out of
/// the download directory or break out of a quoted header.
#[must_use]
pub fn sanitize_download_filename(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.trim().chars() {
        if c.is_control() {
            continue;
        }
        match c {
            '/' | '\\' | '"' | '\'' => out.push('_'),
            _ => out.push(c),
        }
    }
    out.trim_matches('.').trim().to_string()
}

#[cfg(test)]
mod log_name_tests {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::ExportFormat;

    // The log name lands in log lines, metric labels and filenames, so a shared
    // or empty name makes one export indistinguishable from another in a log the
    // only way anyone will read about a failed download.
    #[test]
    fn each_format_has_its_own_log_name() {
        assert_eq!(ExportFormat::Csv.log_name(), "csv");
        assert_eq!(ExportFormat::Json.log_name(), "json");
        assert_eq!(ExportFormat::Rdf.log_name(), "rdf");
    }

    #[test]
    fn the_log_name_is_the_extension_the_download_is_saved_under() {
        // The extension comes from the log name and the Content-Type from
        // `content_type`, so a mismatch is a file the browser cannot read.
        for format in [ExportFormat::Csv, ExportFormat::Json, ExportFormat::Rdf] {
            let name = format.log_name();
            assert!(!name.is_empty(), "{format:?} needs a name");
            assert!(
                format.content_type().contains(name) || name == "rdf",
                "{format:?}: {} does not describe {name}",
                format.content_type()
            );
        }
    }
}

#[cfg(test)]
mod tests {
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
}
