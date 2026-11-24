// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The Wikidata archive: fetch LOTUS compound SMILES from `QLever`,
//! deduplicate by `InChIKey`, and package as tar.gz with manifest and checksums.
//!
//! The pipeline runs the same queries as `lotus-wikidata-archive`, but through
//! an injected [`lotus_search::Http`] transport so a test can drive it against a scripted
//! double instead of the network. Every page is fetched once, records are
//! joined in memory before the output is written.
//!
//! No clock: the caller supplies `fetched_at` (the ISO-8601 stamp). No file
//! I/O: this crate returns bytes and strings; the CLI writes them to disk.
//! No Zenodo, no `smiles-rs`: those are separate concerns with their own
//! dependencies and lifecycles. SMILES validation is omitted because `smiles-rs`
//! has no wasm32 story and the workspace lints deny `unwrap`/`expect` on
//! untrusted parser output; `parse_errors` is always zero in this port.

mod dedup;
mod fetch;
mod tar_writer;

use lotus_search::{Endpoint, FetchError, Http};
use serde::Serialize;
use thiserror::Error;

use self::dedup::deduplicate_by_inchi_key;
use self::fetch::QleverClient;
pub use self::tar_writer::{WriteArchiveInput, write_archive};

/// `QLever`'s public Wikidata SPARQL endpoint, the default for the archive.
pub const DEFAULT_QLEVER_ENDPOINT: &str = lotus_model::QLEVER_WIKIDATA;

/// The Wikidata property from which a SMILES value was obtained.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SmilesKind {
    /// Wikidata P233, canonical SMILES.
    Canonical,
    /// Wikidata P2017, isomeric SMILES.
    Isomeric,
}

impl SmilesKind {
    pub(super) fn parse(value: &str) -> Result<Self, ArchiveError> {
        match value {
            "canonical" => Ok(Self::Canonical),
            "isomeric" => Ok(Self::Isomeric),
            _ => Err(ArchiveError::UnsupportedSmilesKind(value.to_string())),
        }
    }
}

/// One verbatim SMILES statement from a Wikidata chemical compound.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct SmilesRecord {
    /// Standard `InChIKey` (Wikidata `P235`), used as the compound identity.
    pub inchi_key: String,
    /// Wikidata entity ID, e.g. `Q153`, supplying this source value.
    pub wikidata_id: String,
    /// Whether this is P233 or P2017.
    pub smiles_kind: SmilesKind,
    /// The unmodified Wikidata string value.
    pub smiles: String,
}

/// Source SMILES with `P703` but no `P235` `InChIKey`.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MissingInchiKeyRecord {
    /// Wikidata entity ID, e.g. `Q153`.
    pub wikidata_id: String,
    /// Whether this is P233 or P2017.
    pub smiles_kind: SmilesKind,
    /// The unmodified Wikidata string value.
    pub smiles: String,
}

/// A SMILES parse failure included beside the source archive.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ParseErrorRecord {
    /// Standard `InChIKey` identifying the deduplication group.
    pub inchi_key: String,
    /// Wikidata entity ID.
    pub wikidata_id: String,
    /// Property represented by the failed value.
    pub smiles_kind: SmilesKind,
    /// The source string that failed to parse.
    pub smiles: String,
    /// Why the parser rejected it. The archive header has an `error` column and
    /// the CSV writer refuses a record shorter than its header.
    pub error: String,
}

/// The summary the archive returns: what it fetched and wrote.
#[derive(Debug, Default)]
pub struct ArchiveSummary {
    /// How many source values with an `InChIKey` were fetched before deduplication.
    pub source_records: usize,
    /// How many source values were excluded because `P235` is absent.
    pub missing_inchi_key_records: usize,
    /// How many selected records remain after `InChIKey` deduplication.
    pub records: usize,
    /// How many parse errors were recorded.
    pub parse_errors: usize,
}

/// Parameters controlling a [`run_archive`] run.
#[derive(Debug, Clone)]
pub struct ArchiveOptions<'a> {
    /// `QLever` SPARQL endpoint to query.
    pub endpoint: Endpoint,
    /// Caller-supplied ISO-8601 UTC timestamp recorded in `manifest.json`.
    pub fetched_at: &'a str,
    /// File name recorded inside `SHA256SUMS` so `sha256sum -c` verifies cleanly.
    pub archive_filename: &'a str,
}

/// Errors the archive can report, never panic on.
#[derive(Debug, Error)]
pub enum ArchiveError {
    /// The endpoint or transport failed to answer.
    #[error("fetch failed: {0}")]
    Fetch(#[from] FetchError),
    /// The payload could not be read as JSON.
    #[error("JSON parse failed: {0}")]
    Json(#[from] serde_json::Error),
    /// Writing the archive failed.
    #[error("I/O failed: {0}")]
    Io(#[from] std::io::Error),
    /// CSV serialization failed.
    #[error("CSV serialize failed: {0}")]
    Csv(#[from] csv::Error),
    /// A result set's header did not name a column the parser needs.
    #[error("the `{table}` result lacks the `{column}` column")]
    MissingColumn {
        /// The result set that was missing the column.
        table: &'static str,
        /// The column the header did not name.
        column: &'static str,
    },
    /// `QLever` returned an unsupported SMILES kind.
    #[error("unsupported SMILES kind: {0}")]
    UnsupportedSmilesKind(String),
}

/// Paths emitted for one source snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveArtifacts {
    /// Gzip-compressed tar archive bytes.
    pub archive: Vec<u8>,
    /// Standalone copy of the archive manifest JSON.
    pub manifest: String,
    /// SHA-256 digests of the archive and standalone manifest.
    pub checksums: String,
}

/// Run the full Wikidata archive: fetch every page of both SMILES queries,
/// deduplicate by `InChIKey`, and package as tar.gz plus manifest and checksums.
///
/// Returns the tar.gz bytes, the manifest JSON, and the SHA-256 checksums
/// string. The caller writes them to disk.
///
/// # Errors
/// Returns [`ArchiveError`] when a fetch, a parse, or a serialization fails.
pub async fn run_archive<H: Http>(
    http: &H,
    options: &ArchiveOptions<'_>,
) -> Result<(ArchiveArtifacts, ArchiveSummary), ArchiveError> {
    let client = QleverClient::new(http, options.endpoint.clone());

    // 1. Fetch every page of both archive queries.
    let source_records = client.fetch_lotus_smiles().await?;
    let source_records_count = source_records.len();
    let missing_inchi_key_records = client.fetch_missing_inchi_key_smiles().await?;

    // 2. Deduplicate by InChIKey (preferring isomeric P2017 over canonical P233).
    let records = deduplicate_by_inchi_key(source_records);

    // SMILES validation is omitted: `smiles-rs` has no wasm32 story. Parse
    // errors are empty; the archive still carries the count for format
    // compatibility with the Python lotus-wikidata-archive.
    let parse_errors: Vec<ParseErrorRecord> = Vec::new();

    let summary = ArchiveSummary {
        source_records: source_records_count,
        missing_inchi_key_records: missing_inchi_key_records.len(),
        records: records.len(),
        parse_errors: parse_errors.len(),
    };

    let artifacts = write_archive(&WriteArchiveInput {
        fetched_at: options.fetched_at,
        archive_filename: options.archive_filename,
        endpoint: options.endpoint.url(),
        source_records: source_records_count,
        missing_inchi_key_records: &missing_inchi_key_records,
        records: &records,
        parse_errors: &parse_errors,
    })?;

    Ok((artifacts, summary))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
