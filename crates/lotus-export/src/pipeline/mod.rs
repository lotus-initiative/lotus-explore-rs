// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The LOTUS exporter pipeline: fetch Wikidata tables, join them, write the
//! export files.
//!
//! The pipeline runs the same queries as `lotus_exporter.py`, but through an
//! injected [`lotus_search::Http`] transport so a test can drive it against a scripted
//! double instead of the network. Every table is fetched once, indexed by QID,
//! and joined in memory, and the output is built in memory too: nothing here
//! streams to disk.
//!
//! No clock: the caller supplies `now` (the ISO-8601 stamp) and `year`. No
//! UI framework: this crate draws nothing. The CLI supplies its own runtime and
//! file writers.

mod index;
mod join;

use std::collections::HashMap;
use std::sync::Arc;

use lotus_query::{
    COMPOUND_TAXON_REFERENCE_QUERY, EXPORT_COMPOUNDS_QUERY, EXPORT_REFERENCES_QUERY,
    EXPORT_TAXA_QUERY, extract_qid,
};
use lotus_search::{Endpoint, FetchError, Http, ResponseFormat, execute_streaming};
use thiserror::Error;

use self::index::{build_compound_index, build_reference_index, build_taxon_index, read_triplets};
use self::join::write_joined_rows;

/// One result row, as column name → cell value.
pub(crate) type Row = HashMap<Arc<str>, String>;

/// Number of sequential table requests in a full export.
pub const EXPORT_QUERY_COUNT: u64 = 4;

/// Parameters controlling a [`run_lotus_exporter`] run.
#[derive(Debug, Clone)]
pub struct PipelineOptions<'a> {
    /// SPARQL endpoint to query.
    pub endpoint: Endpoint,
    /// Caller-supplied ISO-8601 UTC timestamp recorded in `frozen_metadata.csv`.
    pub now: &'a str,
    /// Caller-supplied calendar year recorded in `frozen_metadata.csv`.
    pub year: u16,
}

/// One line of `frozen_metadata.csv`: a `key,value` pair.
#[derive(Clone)]
struct MetaLine {
    key: String,
    value: String,
}

/// The summary the pipeline returns: what it fetched and wrote.
#[derive(Debug, Default)]
pub struct ExportSummary {
    /// How many compound-taxon-reference triplet rows were fetched.
    pub triplets: usize,
    /// How many distinct compounds were indexed from the structure tables.
    pub compounds: usize,
    /// How many distinct taxa were indexed from the taxon tables.
    pub taxa: usize,
    /// How many distinct references were indexed from the reference tables.
    pub references: usize,
    /// How many joined rows were written to `frozen.csv`.
    pub rows_written: usize,
}

/// Errors the pipeline can report, never panic on.
#[derive(Debug, Error)]
pub enum ExportError {
    /// The endpoint or transport failed to answer.
    #[error("fetch failed: {0}")]
    Fetch(#[from] FetchError),
    /// The payload could not be read as CSV.
    #[error("CSV parse failed: {0}")]
    Csv(#[from] csv::Error),
    /// Writing a file failed.
    #[error("I/O failed: {0}")]
    Io(#[from] std::io::Error),
    /// A result set's header did not name a column the join needs.
    #[error("the `{table}` result lacks the `{column}` column")]
    MissingColumn {
        /// The result set that was missing the column.
        table: &'static str,
        /// The column the header did not name.
        column: &'static str,
    },
}

/// Read one SPARQL CSV payload into row maps.
pub(crate) fn read_csv_rows(bytes: &[u8]) -> Result<Vec<Row>, csv::Error> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(bytes);
    let headers: Vec<Arc<str>> = reader.headers()?.iter().map(Arc::from).collect();
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        let mut row = HashMap::with_capacity(headers.len());
        for (i, header) in headers.iter().enumerate() {
            row.insert(
                Arc::clone(header),
                record.get(i).map_or_else(String::new, str::to_string),
            );
        }
        rows.push(row);
    }
    Ok(rows)
}

/// POST one SPARQL query and collect its streamed CSV bytes.
async fn fetch_csv_bytes<H: Http>(
    http: &H,
    endpoint: &Endpoint,
    query: &str,
) -> Result<Vec<u8>, ExportError> {
    let mut answer = execute_streaming(http, endpoint.clone(), query, ResponseFormat::Csv).await?;
    let mut body = Vec::new();
    while let Some(bytes) = answer.chunks.next_chunk().await? {
        body.extend_from_slice(&bytes);
    }
    Ok(body)
}

/// POST one SPARQL query, read every chunk, and parse the CSV into rows.
pub(crate) async fn fetch_csv_rows<H: Http>(
    http: &H,
    endpoint: &Endpoint,
    query: &str,
    _table: &'static str,
) -> Result<Vec<Row>, ExportError> {
    let body = fetch_csv_bytes(http, endpoint, query).await?;
    read_csv_rows(&body).map_err(ExportError::Csv)
}

/// Notify the caller before a table fetch, leaving presentation to its adapter.
async fn fetch_export_bytes<H: Http>(
    http: &H,
    endpoint: &Endpoint,
    query: &str,
    table: &'static str,
    on_progress: &mut impl FnMut(&'static str),
) -> Result<Vec<u8>, ExportError> {
    on_progress(table);
    fetch_csv_bytes(http, endpoint, query).await
}

/// The value of one column, empty when absent.
pub(crate) fn cell<'a>(row: &'a Row, column: &str) -> &'a str {
    row.get(column).map_or("", String::as_str)
}

/// The QID in one entity column, stripped of its Wikidata entity IRI prefix.
pub(crate) fn qid(row: &Row, column: &str) -> String {
    extract_qid(cell(row, column)).to_string()
}

/// Serialize `frozen_metadata.csv` as `key,value` lines into a `String`.
fn serialize_metadata_csv(lines: &[MetaLine]) -> Result<String, ExportError> {
    let mut buffer = Vec::new();
    {
        let mut writer = csv::Writer::from_writer(&mut buffer);
        for line in lines {
            writer.write_record([&line.key, &line.value])?;
        }
        writer.flush()?;
    }
    String::from_utf8(buffer)
        .map_err(|e| ExportError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))
}

/// Run the full LOTUS export: fetch every table, index by QID, join, and
/// serialize `frozen.csv` plus `frozen_metadata.csv`.
///
/// `on_progress` is called before each of the [`EXPORT_QUERY_COUNT`] table
/// requests with its name.
///
/// The fetched tables, their indexes and the finished CSV are all in memory
/// at once, so peak memory grows with the graph. The caller writes the returned
/// strings to disk; this crate performs no file I/O, so it stays wasm-clean.
///
/// # Errors
/// Returns [`ExportError`] when a fetch, a parse, or a serialization fails.
pub async fn run_lotus_exporter<H: Http>(
    http: &H,
    options: &PipelineOptions<'_>,
    mut on_progress: impl FnMut(&'static str),
) -> Result<(String, String, ExportSummary), ExportError> {
    let triplet_bytes = fetch_export_bytes(
        http,
        &options.endpoint,
        COMPOUND_TAXON_REFERENCE_QUERY,
        "compound_taxon_reference",
        &mut on_progress,
    )
    .await?;
    let triplets = read_triplets(&triplet_bytes)?;
    let triplet_count = triplets.len();

    let compound_bytes = fetch_export_bytes(
        http,
        &options.endpoint,
        EXPORT_COMPOUNDS_QUERY,
        "compounds",
        &mut on_progress,
    )
    .await?;
    let compounds = build_compound_index(&compound_bytes)?;

    let taxon_bytes = fetch_export_bytes(
        http,
        &options.endpoint,
        EXPORT_TAXA_QUERY,
        "taxa",
        &mut on_progress,
    )
    .await?;
    let taxa = build_taxon_index(&taxon_bytes)?;

    let reference_bytes = fetch_export_bytes(
        http,
        &options.endpoint,
        EXPORT_REFERENCES_QUERY,
        "references",
        &mut on_progress,
    )
    .await?;
    let references = build_reference_index(&reference_bytes)?;

    let mut buffer = Vec::new();
    let rows_written = write_joined_rows(&triplets, &compounds, &taxa, &references, &mut buffer)?;

    let frozen_csv = String::from_utf8(buffer)
        .map_err(|e| ExportError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;

    let meta_lines = vec![
        MetaLine {
            key: "endpoint".into(),
            value: options.endpoint.url().to_string(),
        },
        MetaLine {
            key: "date_created".into(),
            value: options.now.to_string(),
        },
        MetaLine {
            key: "year".into(),
            value: options.year.to_string(),
        },
        MetaLine {
            key: "triplets".into(),
            value: triplet_count.to_string(),
        },
        MetaLine {
            key: "rows_written".into(),
            value: rows_written.to_string(),
        },
    ];
    let metadata_csv = serialize_metadata_csv(&meta_lines)?;

    Ok((
        frozen_csv,
        metadata_csv,
        ExportSummary {
            triplets: triplet_count,
            compounds: compounds.len(),
            taxa: taxa.len(),
            references: references.len(),
            rows_written,
        },
    ))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
