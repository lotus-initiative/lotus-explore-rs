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
    ///
    /// Deliberately [`Self::extension`] rather than a second copy of the same
    /// match. The two were written out separately and had drifted into being
    /// character-for-character identical, which is what `cargo dejadoc` reports:
    /// three formats whose log name happens to be their extension. If a format is
    /// ever added whose log name is *not* its extension -- `ndjson` and `json`
    /// being the obvious candidate -- this becomes a real function again, and the
    /// gate that flagged the copy is what should prompt that change.
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
