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
    /// "ndjson", "ttl"). Returns `None` for anything else.
    ///
    /// `rdf` is accepted as well as `ttl`, and kept for links rather than for
    /// taste. This name is a URL path segment, so renaming it outright breaks
    /// every export link already handed out and every one cached by a browser;
    /// parsing the old name costs one arm and leaves those working.
    ///
    /// `turtle` is *not* accepted. The set of names a URL accepts is the
    /// documented set, and a format that is not in the documentation should be
    /// added to the documentation first.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "csv" => Some(Self::Csv),
            "json" | "ndjson" => Some(Self::Json),
            "ttl" | "rdf" => Some(Self::Rdf),
            _ => None,
        }
    }

    /// File extension without a leading dot.
    ///
    /// Turtle is named for what it is, so this is `ttl`. The previous `rdf` is
    /// the conventional extension for RDF/XML, which this never produces: the
    /// content type is `text/turtle` and the endpoint action is
    /// `turtle_export`, so a reader handed a `.rdf` was handed a file whose
    /// extension promised a serialization it did not contain.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
            Self::Rdf => "ttl",
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

    /// Short name for a log line or a metric label.
    ///
    /// Deliberately [`Self::extension`] rather than a second copy of the same
    /// match. The two were written out separately and had drifted into being
    /// character-for-character identical, which is what `cargo dejadoc` reports.
    /// If a format is ever added whose log name is *not* its extension --
    /// `ndjson` and `json` being the obvious candidate -- this becomes a real
    /// function again, and the gate that flagged the copy is what should prompt
    /// that change.
    ///
    /// So this is `ttl` for Turtle and not a second word for it: the format was
    /// renamed, and a log line disagreeing with the extension is the drift this
    /// function exists to prevent. That also renames the telemetry series from
    /// `rdf` to `ttl`, which is the same event under a new name rather than a
    /// second event.
    #[must_use]
    pub const fn log_name(self) -> &'static str {
        self.extension()
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
#[path = "export/log_name_tests.rs"]
mod log_name_tests;

#[cfg(test)]
#[path = "export/tests.rs"]
mod tests;
