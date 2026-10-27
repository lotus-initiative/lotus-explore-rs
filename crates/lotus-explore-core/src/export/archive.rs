// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The Wikidata archive: fetch LOTUS compound SMILES from QLever,
//! deduplicate by `InChIKey`, and package as tar.gz with manifest and checksums.
//!
//! The pipeline runs the same queries as `lotus-wikidata-archive`, but through
//! an injected [`Http`] transport so a test can drive it against a scripted
//! double instead of the network. Every page is fetched once, records are
//! joined in memory before the output is written.
//!
//! No clock: the caller supplies `fetched_at` (the ISO-8601 stamp). No file
//! I/O: this crate returns bytes and strings; the CLI writes them to disk.
//! No Zenodo, no `smiles-rs`: those are separate concerns with their own
//! dependencies and lifecycles. SMILES validation is omitted because `smiles-rs`
//! has no wasm32 story and the workspace lints deny `unwrap`/`expect` on
//! untrusted parser output; `parse_errors` is always zero in this port.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use flate2::Compression;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tar::Builder;

use lotus_query::{
    ARCHIVE_PAGE_SIZE, ARCHIVE_SMILES_QUERY, ARCHIVE_SMILES_WITHOUT_INCHIKEY_QUERY, archive_page,
};
use lotus_search::{Endpoint, FetchError, Http};
use thiserror::Error;

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
    fn parse(value: &str) -> Result<Self, ArchiveError> {
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

/// `QLever`'s public Wikidata SPARQL endpoint, the default for the archive.
pub const DEFAULT_QLEVER_ENDPOINT: &str = "https://qlever.dev/api/wikidata";

/// SPARQL JSON response shape for the archive queries.
#[derive(Deserialize)]
struct SparqlResponse {
    results: SparqlResults,
}

#[derive(Deserialize)]
struct SparqlResults {
    bindings: Vec<SparqlBinding>,
}

#[derive(Deserialize)]
struct SparqlBinding {
    compound: SparqlValue,
    #[serde(rename = "inchiKey")]
    inchi_key: Option<SparqlValue>,
    smiles: SparqlValue,
    #[serde(rename = "smilesKind")]
    smiles_kind: SparqlValue,
}

#[derive(Deserialize)]
struct SparqlValue {
    value: String,
}

/// HTTP client for fetching LOTUS SMILES from `QLever`.
#[derive(Clone, Debug)]
pub struct QleverClient<'a, H: Http + 'a> {
    http: &'a H,
    endpoint: Endpoint,
}

impl<'a, H: Http + 'a> QleverClient<'a, H> {
    /// Constructs a client for the supplied `QLever` SPARQL endpoint.
    pub const fn new(http: &'a H, endpoint: Endpoint) -> Self {
        Self { http, endpoint }
    }

    /// Executes every stable page of [`ARCHIVE_SMILES_QUERY`] and returns the
    /// already ordered, duplicate-free source records.
    ///
    /// # Errors
    /// Returns [`ArchiveError::Fetch`] when the endpoint fails to answer.
    pub async fn fetch_lotus_smiles(&self) -> Result<Vec<SmilesRecord>, ArchiveError> {
        decode_sparql_bindings(self.fetch_bindings(ARCHIVE_SMILES_QUERY).await?)
    }

    /// Retrieves source values that cannot be deduplicated because `P235` is absent.
    ///
    /// # Errors
    /// Returns [`ArchiveError::Fetch`] when the endpoint fails to answer.
    pub async fn fetch_missing_inchi_key_smiles(
        &self,
    ) -> Result<Vec<MissingInchiKeyRecord>, ArchiveError> {
        decode_missing_inchi_key_bindings(
            self.fetch_bindings(ARCHIVE_SMILES_WITHOUT_INCHIKEY_QUERY)
                .await?,
        )
    }

    async fn fetch_bindings(&self, base_query: &str) -> Result<Vec<SparqlBinding>, ArchiveError> {
        let mut bindings = Vec::new();
        let mut offset = 0;

        loop {
            let page = self.fetch_page(base_query, offset).await?;
            let page_len = page.len();
            bindings.extend(page);

            if page_len < ARCHIVE_PAGE_SIZE {
                return Ok(bindings);
            }
            offset = offset.checked_add(ARCHIVE_PAGE_SIZE).ok_or_else(|| {
                ArchiveError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "QLever page offset overflow",
                ))
            })?;
        }
    }

    async fn fetch_page(
        &self,
        base_query: &str,
        offset: usize,
    ) -> Result<Vec<SparqlBinding>, ArchiveError> {
        let query = archive_page(base_query, offset);
        let answer = lotus_search::execute(
            self.http,
            self.endpoint.clone(),
            &query,
            lotus_search::ResponseFormat::SparqlJson,
        )
        .await
        .map_err(ArchiveError::Fetch)?;
        let text = answer.text().map_err(ArchiveError::Fetch)?;
        let response: SparqlResponse = serde_json::from_str(&text).map_err(ArchiveError::Json)?;
        Ok(response.results.bindings)
    }
}

fn decode_sparql_bindings(bindings: Vec<SparqlBinding>) -> Result<Vec<SmilesRecord>, ArchiveError> {
    bindings
        .into_iter()
        .map(|binding| {
            let wikidata_id = wikidata_id(&binding.compound.value)?;
            let inchi_key = binding
                .inchi_key
                .ok_or(ArchiveError::MissingColumn {
                    table: "smiles",
                    column: "inchiKey",
                })?
                .value;
            Ok(SmilesRecord {
                inchi_key,
                wikidata_id,
                smiles_kind: SmilesKind::parse(&binding.smiles_kind.value)?,
                smiles: binding.smiles.value,
            })
        })
        .collect()
}

fn decode_missing_inchi_key_bindings(
    bindings: Vec<SparqlBinding>,
) -> Result<Vec<MissingInchiKeyRecord>, ArchiveError> {
    bindings
        .into_iter()
        .map(|binding| {
            if binding.inchi_key.is_some() {
                return Err(ArchiveError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "QLever missing-InChIKey result unexpectedly includes P235",
                )));
            }
            Ok(MissingInchiKeyRecord {
                wikidata_id: wikidata_id(&binding.compound.value)?,
                smiles_kind: SmilesKind::parse(&binding.smiles_kind.value)?,
                smiles: binding.smiles.value,
            })
        })
        .collect()
}

fn wikidata_id(uri: &str) -> Result<String, ArchiveError> {
    uri.rsplit('/')
        .next()
        .filter(|id| id.starts_with('Q'))
        .ok_or_else(|| {
            ArchiveError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "QLever compound is not a Wikidata entity URI",
            ))
        })
        .map(ToOwned::to_owned)
}

/// Selects one record per `InChIKey`, preferring P2017 isomeric SMILES over P233.
///
/// Multiple values of the preferred property are resolved deterministically by
/// source SMILES then Wikidata ID, so repeated pulls yield the same archive.
#[must_use]
pub fn deduplicate_by_inchi_key(records: Vec<SmilesRecord>) -> Vec<SmilesRecord> {
    let mut selected = BTreeMap::new();
    for record in records {
        let inchi_key = record.inchi_key.clone();
        match selected.entry(inchi_key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(record);
            }
            std::collections::btree_map::Entry::Occupied(mut entry)
                if preferred_over(&record, entry.get()) =>
            {
                entry.insert(record);
            }
            std::collections::btree_map::Entry::Occupied(_) => {}
        }
    }
    selected.into_values().collect()
}

fn preferred_over(candidate: &SmilesRecord, current: &SmilesRecord) -> bool {
    let candidate_priority = matches!(candidate.smiles_kind, SmilesKind::Isomeric);
    let current_priority = matches!(current.smiles_kind, SmilesKind::Isomeric);
    (candidate_priority && !current_priority)
        || (candidate_priority == current_priority
            && (candidate.smiles.as_str(), candidate.wikidata_id.as_str())
                < (current.smiles.as_str(), current.wikidata_id.as_str()))
}

/// The manifest written into the archive and as a standalone file.
#[derive(Serialize)]
struct Manifest<'a> {
    archive_format: &'static str,
    fetched_at: String,
    qlever_endpoint: &'a str,
    /// SPARQL query selecting source records with `P235`.
    query: &'a str,
    /// SPARQL query used to find entries excluded for missing `P235`.
    missing_inchi_key_query: &'static str,
    /// Maximum result rows requested by each stable query page.
    page_size: usize,
    /// Number of source values with an `InChIKey` before deduplication.
    source_records: usize,
    /// Number of source values excluded because `P235` is absent.
    missing_inchi_key_records: usize,
    /// Number of selected records after `InChIKey` deduplication.
    records: usize,
    parse_errors: usize,
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

/// Writes a gzip-compressed tar archive and a standalone manifest beside it.
///
/// The archive members have a fixed metadata timestamp so the only intended
/// run-specific value is `manifest.json`'s `fetched_at` field.
///
/// # Errors
/// Returns [`ArchiveError::Csv`] or [`ArchiveError::Io`] when serialization fails.
pub fn write_archive(
    fetched_at: &str,
    endpoint: &str,
    source_records: usize,
    missing_inchi_key_records: &[MissingInchiKeyRecord],
    records: &[SmilesRecord],
    parse_errors: &[ParseErrorRecord],
) -> Result<ArchiveArtifacts, ArchiveError> {
    let records_csv = records_csv_bytes(records)?;
    let missing_inchi_key_csv = missing_inchi_key_csv_bytes(missing_inchi_key_records)?;
    let errors_csv = parse_errors_csv_bytes(parse_errors)?;
    let manifest_bytes = serde_json::to_vec_pretty(&Manifest {
        archive_format: "lotus-wikidata-smiles/v1",
        fetched_at: fetched_at.to_string(),
        qlever_endpoint: endpoint,
        query: ARCHIVE_SMILES_QUERY,
        missing_inchi_key_query: ARCHIVE_SMILES_WITHOUT_INCHIKEY_QUERY,
        page_size: ARCHIVE_PAGE_SIZE,
        source_records,
        missing_inchi_key_records: missing_inchi_key_records.len(),
        records: records.len(),
        parse_errors: parse_errors.len(),
    })?;

    let archive_bytes = {
        let mut buffer = Vec::new();
        {
            let mut archive = Builder::new(GzEncoder::new(&mut buffer, Compression::default()));
            append_member(&mut archive, "lotus-wikidata-smiles.csv", &records_csv)?;
            append_member(&mut archive, "missing-inchikey.csv", &missing_inchi_key_csv)?;
            append_member(&mut archive, "smiles-parse-errors.csv", &errors_csv)?;
            append_member(&mut archive, "manifest.json", &manifest_bytes)?;
            archive.finish()?;
        }
        // Dropping the Builder and GzEncoder here finalizes tar and gzip.
        buffer
    };

    let manifest = String::from_utf8(manifest_bytes)
        .map_err(|e| ArchiveError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;

    let archive_filename = "lotus-wikidata-smiles.tar.gz";
    let manifest_filename = "lotus-wikidata-smiles.manifest.json";
    let checksums = write_checksums(&[
        (archive_bytes.as_slice(), archive_filename),
        (manifest.as_bytes(), manifest_filename),
    ]);

    Ok(ArchiveArtifacts {
        archive: archive_bytes,
        manifest,
        checksums,
    })
}

fn append_member<W: std::io::Write>(
    archive: &mut Builder<GzEncoder<W>>,
    name: &str,
    contents: &[u8],
) -> Result<(), ArchiveError> {
    let mut header = tar::Header::new_gnu();
    header.set_size(contents.len() as u64);
    header.set_mode(0o644);
    header.set_mtime(0);
    header.set_cksum();
    archive
        .append_data(&mut header, name, contents)
        .map_err(|e| ArchiveError::Io(std::io::Error::other(e)))
}

fn records_csv_bytes(records: &[SmilesRecord]) -> Result<Vec<u8>, ArchiveError> {
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(Vec::new());
    writer.write_record(["inchi_key", "wikidata_id", "smiles_kind", "smiles"])?;
    for record in records {
        writer.serialize(record)?;
    }
    Ok(writer
        .into_inner()
        .map_err(csv::IntoInnerError::into_error)?)
}

fn parse_errors_csv_bytes(records: &[ParseErrorRecord]) -> Result<Vec<u8>, ArchiveError> {
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(Vec::new());
    writer.write_record(["inchi_key", "wikidata_id", "smiles_kind", "smiles", "error"])?;
    for record in records {
        writer.serialize(record)?;
    }
    Ok(writer
        .into_inner()
        .map_err(csv::IntoInnerError::into_error)?)
}

fn missing_inchi_key_csv_bytes(records: &[MissingInchiKeyRecord]) -> Result<Vec<u8>, ArchiveError> {
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(Vec::new());
    writer.write_record(["wikidata_id", "smiles_kind", "smiles"])?;
    for record in records {
        writer.serialize(record)?;
    }
    Ok(writer
        .into_inner()
        .map_err(csv::IntoInnerError::into_error)?)
}

fn write_checksums(files: &[(&[u8], &str)]) -> String {
    let mut output = String::new();
    for (contents, label) in files {
        let mut hasher = Sha256::new();
        hasher.update(contents);
        let digest = hasher.finalize();
        for byte in digest {
            let _ = write!(output, "{byte:02x}");
        }
        output.push_str("  ");
        output.push_str(label);
        output.push('\n');
    }
    output
}

/// Run the full Wikidata archive: fetch every page of both SMILES queries,
/// deduplicate by `InChIKey`, and package as tar.gz plus manifest and checksums.
///
/// `fetched_at` is the ISO-8601 stamp the caller read; `endpoint` is the
/// `QLever` service to query.
///
/// Returns the tar.gz bytes, the manifest JSON, and the SHA-256 checksums
/// string. The caller writes them to disk.
///
/// # Errors
/// Returns [`ArchiveError`] when a fetch, a parse, or a serialization fails.
pub async fn run_archive<H: Http>(
    http: &H,
    endpoint: Endpoint,
    fetched_at: &str,
) -> Result<(ArchiveArtifacts, ArchiveSummary), ArchiveError> {
    let client = QleverClient::new(http, endpoint.clone());

    // 1. Fetch every page of both archive queries.
    let source_records = client.fetch_lotus_smiles().await?;
    let missing_inchi_key_records = client.fetch_missing_inchi_key_smiles().await?;

    // 2. Deduplicate by InChIKey (preferring isomeric P2017 over canonical P233).
    let records = deduplicate_by_inchi_key(source_records.clone());

    // SMILES validation is omitted: `smiles-rs` has no wasm32 story. Parse
    // errors are empty; the archive still carries the count for format
    // compatibility with the Python lotus-wikidata-archive.
    let parse_errors: Vec<ParseErrorRecord> = Vec::new();

    let summary = ArchiveSummary {
        source_records: source_records.len(),
        missing_inchi_key_records: missing_inchi_key_records.len(),
        records: records.len(),
        parse_errors: parse_errors.len(),
    };

    let artifacts = write_archive(
        fetched_at,
        endpoint.url(),
        source_records.len(),
        &missing_inchi_key_records,
        &records,
        &parse_errors,
    )?;

    Ok((artifacts, summary))
}

#[cfg(test)]
#[path = "archive/tests.rs"]
mod tests;
