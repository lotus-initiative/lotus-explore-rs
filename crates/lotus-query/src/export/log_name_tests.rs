// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `export`, in their own file.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use super::ExportFormat;

// The log name lands in log lines, metric labels and filenames, so a shared
// or empty name makes one export indistinguishable from another in a log the
// only way anyone will read about a failed download.
#[test]
fn each_format_has_its_own_log_name() {
    assert_eq!(ExportFormat::Csv.log_name(), "csv");
    assert_eq!(ExportFormat::Json.log_name(), "json");
    assert_eq!(ExportFormat::Rdf.log_name(), "ttl");
}

/// The extension has to name what the bytes are.
///
/// Turtle was `rdf`, which is the conventional extension for RDF/XML -- a
/// serialization this never produces, since the content type is `text/turtle`
/// and the endpoint action is `turtle_export`. A reader handed a `.rdf` was
/// handed a file whose extension promised a format it did not contain, and
/// plenty of tools read the extension before the content.
#[test]
fn the_turtle_extension_names_turtle() {
    assert_eq!(ExportFormat::Rdf.extension(), "ttl");
    assert!(
        ExportFormat::Rdf.content_type().contains("turtle"),
        "the extension and the content type have to agree, or the file is a \
         promise the bytes do not keep: {}",
        ExportFormat::Rdf.content_type()
    );
    assert!(
        ExportFormat::Rdf.qlever_action().contains("turtle"),
        "the endpoint asks for Turtle by that name, so the file should be too"
    );
}

/// The one format whose extension is not spelled the way its content type is.
///
/// `csv` is a substring of `text/csv` and `json` is served as
/// `application/sparql-results+json`. Turtle's type says `turtle` where the file
/// says `ttl`, so the general agreement below cannot be checked by substring for
/// it, and this is where that difference is pinned rather than allowed to look
/// like an oversight at the call site.
#[test]
fn the_extension_agrees_with_the_content_type_for_what_it_can() {
    for format in [ExportFormat::Csv, ExportFormat::Json] {
        assert!(
            format.content_type().contains(format.extension()),
            "{format:?}: {} does not contain {}",
            format.content_type(),
            format.extension()
        );
    }
}

/// Both names parse, so links handed out before the rename keep working.
///
/// The extension is a URL path segment. Renaming it without accepting the old
/// name breaks every export link already sent and every one a browser cached.
#[test]
fn both_the_old_and_the_new_name_are_accepted() {
    assert_eq!(ExportFormat::parse("ttl"), Some(ExportFormat::Rdf));
    assert_eq!(ExportFormat::parse("rdf"), Some(ExportFormat::Rdf));
    assert_eq!(ExportFormat::parse("TTL"), Some(ExportFormat::Rdf));
    assert_eq!(
        ExportFormat::parse("turtle"),
        None,
        "the documented set is the set a URL accepts; a name not in the \
         documentation is a documentation change first"
    );
}

#[test]
fn the_log_name_is_the_extension_the_download_is_saved_under() {
    // The extension comes from the log name and the Content-Type from
    // `content_type`, so a mismatch is a file the browser cannot read.
    for format in [ExportFormat::Csv, ExportFormat::Json, ExportFormat::Rdf] {
        let name = format.log_name();
        assert!(!name.is_empty(), "{format:?} needs a name");
        assert_eq!(
            name,
            format.extension(),
            "{format:?}: the log name and the extension have to be one name, or a \
             log line and the file it names describe different things"
        );
    }
}
