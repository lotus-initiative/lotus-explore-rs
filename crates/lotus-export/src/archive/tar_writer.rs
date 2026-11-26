// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Deterministic `.tar.gz`, manifest JSON, and `SHA256SUMS` serialization.

use std::fmt::Write as _;

use flate2::Compression;
use flate2::write::GzEncoder;
use lotus_query::{ARCHIVE_PAGE_SIZE, ARCHIVE_SMILES_QUERY, ARCHIVE_SMILES_WITHOUT_INCHIKEY_QUERY};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tar::Builder;

use super::{
    ArchiveArtifacts, ArchiveError, MissingInchiKeyRecord, ParseErrorRecord, SmilesRecord,
};

/// Inputs to [`write_archive`].
#[derive(Debug, Clone, Copy)]
pub struct WriteArchiveInput<'a> {
    /// Caller-supplied ISO-8601 UTC timestamp written into `manifest.json`.
    pub fetched_at: &'a str,
    /// Name under which the caller writes the archive (recorded in `SHA256SUMS`).
    pub archive_filename: &'a str,
    /// SPARQL endpoint URL recorded in `manifest.json`.
    pub endpoint: &'a str,
    /// Total source records fetched before deduplication.
    pub source_records: usize,
    /// Source records excluded because `P235` is absent.
    pub missing_inchi_key_records: &'a [MissingInchiKeyRecord],
    /// Selected deduplicated SMILES records.
    pub records: &'a [SmilesRecord],
    /// Recorded SMILES parse errors.
    pub parse_errors: &'a [ParseErrorRecord],
}

/// The manifest written into the archive and as a standalone file.
#[derive(Serialize)]
struct Manifest<'a> {
    archive_format: &'static str,
    fetched_at: &'a str,
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

/// Writes a gzip-compressed tar archive and a standalone manifest beside it.
///
/// The archive members have a fixed metadata timestamp so the only intended
/// run-specific value is `manifest.json`'s `fetched_at` field.
///
/// # Errors
/// Returns [`ArchiveError::Csv`] or [`ArchiveError::Io`] when serialization fails.
pub fn write_archive(input: &WriteArchiveInput<'_>) -> Result<ArchiveArtifacts, ArchiveError> {
    let records_csv = csv_bytes(
        &["inchi_key", "wikidata_id", "smiles_kind", "smiles"],
        input.records,
    )?;
    let missing_inchi_key_csv = csv_bytes(
        &["wikidata_id", "smiles_kind", "smiles"],
        input.missing_inchi_key_records,
    )?;
    let errors_csv = csv_bytes(
        &["inchi_key", "wikidata_id", "smiles_kind", "smiles", "error"],
        input.parse_errors,
    )?;
    let manifest_bytes = serde_json::to_vec_pretty(&Manifest {
        archive_format: "lotus-wikidata-smiles/v1",
        fetched_at: input.fetched_at,
        qlever_endpoint: input.endpoint,
        query: ARCHIVE_SMILES_QUERY,
        missing_inchi_key_query: ARCHIVE_SMILES_WITHOUT_INCHIKEY_QUERY,
        page_size: ARCHIVE_PAGE_SIZE,
        source_records: input.source_records,
        missing_inchi_key_records: input.missing_inchi_key_records.len(),
        records: input.records.len(),
        parse_errors: input.parse_errors.len(),
    })?;

    let archive_bytes = {
        let mut buffer = Vec::new();
        {
            let mut archive = Builder::new(GzEncoder::new(&mut buffer, Compression::default()));
            append_member(&mut archive, "lotus-wikidata-smiles.csv", &records_csv)?;
            append_member(&mut archive, "missing-inchikey.csv", &missing_inchi_key_csv)?;
            append_member(&mut archive, "smiles-parse-errors.csv", &errors_csv)?;
            append_member(&mut archive, "manifest.json", &manifest_bytes)?;
            let encoder = archive.into_inner()?;
            encoder.finish()?;
        }
        buffer
    };

    let manifest = String::from_utf8(manifest_bytes)
        .map_err(|e| ArchiveError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;

    let manifest_filename = "lotus-wikidata-smiles.manifest.json";
    let checksums = write_checksums(&[
        (archive_bytes.as_slice(), input.archive_filename),
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

fn csv_bytes<T: Serialize>(headers: &[&str], records: &[T]) -> Result<Vec<u8>, ArchiveError> {
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(Vec::new());
    writer.write_record(headers)?;
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
