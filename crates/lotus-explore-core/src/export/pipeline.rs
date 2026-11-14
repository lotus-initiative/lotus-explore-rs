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
//! UI framework: this crate draws nothing. The CLI and the app each supply
//! their own runtime and their own file writers.

use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;

use lotus_query::{
    COMPOUND_TAXON_REFERENCE_QUERY, EXPORT_COMPOUNDS_QUERY, EXPORT_REFERENCES_QUERY,
    EXPORT_TAXA_QUERY, extract_qid,
};
use lotus_search::{Endpoint, FetchError, Http, ResponseFormat, execute_streaming};
use thiserror::Error;

/// One result row, as column name → cell value.
pub(crate) type Row = HashMap<Arc<str>, String>;

/// One occurrence triplet from [`COMPOUND_TAXON_REFERENCE_QUERY`], with QIDs
/// already stripped of their entity IRI prefixes.
struct TripletRow {
    compound_qid: String,
    compound_inchikey: String,
    taxon_qid: String,
    reference_qid: String,
}

/// All of one compound's structure properties, keyed by compound QID.
#[derive(Clone, Default)]
struct CompoundProps {
    smiles_can: Option<String>,
    smiles_iso: Option<String>,
    cid: Option<String>,
}

/// All of one taxon's properties, keyed by taxon QID.
#[derive(Clone, Default)]
struct TaxonProps {
    name: Option<String>,
    ncbi: Option<String>,
    ott: Option<String>,
    gbif: Option<String>,
    parent: Option<String>,
    rank: Option<String>,
    rank_label: Option<String>,
}

/// All of one reference's properties, keyed by reference QID.
#[derive(Clone, Default)]
struct ReferenceProps {
    doi: Option<String>,
    pmid: Option<String>,
    pmcid: Option<String>,
    title: Option<String>,
    date: Option<String>,
    journal: Option<String>,
    journal_label: Option<String>,
}

/// The joined export row, in `frozen.csv` column order, borrowing from the
/// triplet row and the property indexes so joining allocates nothing.
#[derive(Clone, Copy)]
struct ExportRow<'a> {
    compound_qid: &'a str,
    compound_inchikey: &'a str,
    compound_smiles_can: &'a str,
    compound_smiles_iso: &'a str,
    compound_cid: &'a str,
    taxon_qid: &'a str,
    taxon_name: &'a str,
    taxon_ncbi: &'a str,
    taxon_ott: &'a str,
    taxon_gbif: &'a str,
    taxon_parent: &'a str,
    taxon_rank: &'a str,
    taxon_rank_label: &'a str,
    reference_qid: &'a str,
    reference_title: &'a str,
    reference_doi: &'a str,
    reference_pmid: &'a str,
    reference_pmcid: &'a str,
    reference_date: &'a str,
    reference_journal: &'a str,
    reference_journal_label: &'a str,
}

/// The columns of `frozen.csv`, in write order.
const FROZEN_COLUMNS: &[&str] = &[
    "compound_qid",
    "compound_inchikey",
    "compound_smiles_can",
    "compound_smiles_iso",
    "compound_cid",
    "taxon_qid",
    "taxon_name",
    "taxon_ncbi",
    "taxon_ott",
    "taxon_gbif",
    "taxon_parent",
    "taxon_rank",
    "taxon_rank_label",
    "reference_qid",
    "reference_title",
    "reference_doi",
    "reference_pmid",
    "reference_pmcid",
    "reference_date",
    "reference_journal",
    "reference_journal_label",
];

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

/// Read a named column from `record`, returning `""` when the column is absent.
fn record_col<'a>(
    headers: &csv::StringRecord,
    record: &'a csv::StringRecord,
    name: &str,
) -> &'a str {
    headers
        .iter()
        .position(|h| h == name)
        .and_then(|i| record.get(i))
        .unwrap_or("")
}

/// Overwrite `slot` when `record` carries a non-empty value for `name`.
fn set_col(
    slot: &mut Option<String>,
    headers: &csv::StringRecord,
    record: &csv::StringRecord,
    name: &str,
) {
    let val = record_col(headers, record, name);
    if !val.is_empty() {
        *slot = Some(val.to_string());
    }
}

/// Index a CSV payload by QID without allocating per-row `HashMap`s.
fn index_csv_records<V: Default>(
    bytes: &[u8],
    key_col: &str,
    mut populate: impl FnMut(&mut V, &csv::StringRecord, &csv::StringRecord),
) -> Result<HashMap<String, V>, csv::Error> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    let mut index = HashMap::new();
    for record in reader.records() {
        let record = record?;
        let key = extract_qid(record_col(&headers, &record, key_col)).to_string();
        populate(index.entry(key).or_default(), &headers, &record);
    }
    Ok(index)
}

/// Parse the occurrence triplets table directly into [`TripletRow`] records.
fn read_triplets(bytes: &[u8]) -> Result<Vec<TripletRow>, csv::Error> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        rows.push(TripletRow {
            compound_qid: extract_qid(record_col(&headers, &record, "compound")).to_string(),
            compound_inchikey: record_col(&headers, &record, "compound_inchikey").to_string(),
            taxon_qid: extract_qid(record_col(&headers, &record, "taxon")).to_string(),
            reference_qid: extract_qid(record_col(&headers, &record, "reference")).to_string(),
        });
    }
    Ok(rows)
}

/// Build `compound QID → CompoundProps` from the consolidated compounds CSV.
fn build_compound_index(bytes: &[u8]) -> Result<HashMap<String, CompoundProps>, csv::Error> {
    index_csv_records(
        bytes,
        "compound",
        |entry: &mut CompoundProps, headers, record| {
            set_col(
                &mut entry.smiles_can,
                headers,
                record,
                "compound_smiles_can",
            );
            set_col(
                &mut entry.smiles_iso,
                headers,
                record,
                "compound_smiles_iso",
            );
            set_col(&mut entry.cid, headers, record, "compound_cid");
        },
    )
}

/// Build `taxon QID → TaxonProps` from the consolidated taxa CSV.
fn build_taxon_index(bytes: &[u8]) -> Result<HashMap<String, TaxonProps>, csv::Error> {
    index_csv_records(bytes, "taxon", |entry: &mut TaxonProps, headers, record| {
        set_col(&mut entry.name, headers, record, "taxon_name");
        set_col(&mut entry.ncbi, headers, record, "taxon_ncbi");
        set_col(&mut entry.ott, headers, record, "taxon_ott");
        set_col(&mut entry.gbif, headers, record, "taxon_gbif");
        set_col(&mut entry.parent, headers, record, "taxon_parent");
        set_col(&mut entry.rank, headers, record, "taxon_rank");
        set_col(&mut entry.rank_label, headers, record, "taxon_rank_label");
    })
}

/// Build `reference QID → ReferenceProps` from the consolidated references CSV.
fn build_reference_index(bytes: &[u8]) -> Result<HashMap<String, ReferenceProps>, csv::Error> {
    index_csv_records(
        bytes,
        "reference",
        |entry: &mut ReferenceProps, headers, record| {
            set_col(&mut entry.doi, headers, record, "doi");
            set_col(&mut entry.pmid, headers, record, "pmid");
            set_col(&mut entry.pmcid, headers, record, "pmcid");
            set_col(&mut entry.title, headers, record, "title");
            set_col(&mut entry.date, headers, record, "date");
            set_col(&mut entry.journal, headers, record, "journal");
            set_col(&mut entry.journal_label, headers, record, "journal_label");
        },
    )
}

/// Join every triplet and stream the resulting rows to `out` as a CSV,
/// writing each row as soon as it is produced instead of buffering them all.
///
/// Returns the number of rows written.
fn write_joined_rows<W: Write>(
    triplets: &[TripletRow],
    compounds: &HashMap<String, CompoundProps>,
    taxa: &HashMap<String, TaxonProps>,
    references: &HashMap<String, ReferenceProps>,
    out: &mut W,
) -> Result<usize, ExportError> {
    let mut writer = csv::Writer::from_writer(out);
    writer.write_record(FROZEN_COLUMNS)?;
    let mut count = 0;
    for triplet in triplets {
        let compound_qid = triplet.compound_qid.as_str();
        let taxon_qid = triplet.taxon_qid.as_str();
        let reference_qid = triplet.reference_qid.as_str();

        let compound = compounds.get(compound_qid);
        let taxon = taxa.get(taxon_qid);
        let reference = references.get(reference_qid);

        let row = ExportRow {
            compound_qid,
            compound_inchikey: triplet.compound_inchikey.as_str(),
            compound_smiles_can: compound.and_then(|c| c.smiles_can.as_deref()).unwrap_or(""),
            compound_smiles_iso: compound.and_then(|c| c.smiles_iso.as_deref()).unwrap_or(""),
            compound_cid: compound.and_then(|c| c.cid.as_deref()).unwrap_or(""),
            taxon_qid,
            taxon_name: taxon.and_then(|t| t.name.as_deref()).unwrap_or(""),
            taxon_ncbi: taxon.and_then(|t| t.ncbi.as_deref()).unwrap_or(""),
            taxon_ott: taxon.and_then(|t| t.ott.as_deref()).unwrap_or(""),
            taxon_gbif: taxon.and_then(|t| t.gbif.as_deref()).unwrap_or(""),
            taxon_parent: taxon.and_then(|t| t.parent.as_deref()).unwrap_or(""),
            taxon_rank: taxon.and_then(|t| t.rank.as_deref()).unwrap_or(""),
            taxon_rank_label: taxon.and_then(|t| t.rank_label.as_deref()).unwrap_or(""),
            reference_qid,
            reference_title: reference.and_then(|r| r.title.as_deref()).unwrap_or(""),
            reference_doi: reference.and_then(|r| r.doi.as_deref()).unwrap_or(""),
            reference_pmid: reference.and_then(|r| r.pmid.as_deref()).unwrap_or(""),
            reference_pmcid: reference.and_then(|r| r.pmcid.as_deref()).unwrap_or(""),
            reference_date: reference.and_then(|r| r.date.as_deref()).unwrap_or(""),
            reference_journal: reference.and_then(|r| r.journal.as_deref()).unwrap_or(""),
            reference_journal_label: reference
                .and_then(|r| r.journal_label.as_deref())
                .unwrap_or(""),
        };
        let record = [
            row.compound_qid,
            row.compound_inchikey,
            row.compound_smiles_can,
            row.compound_smiles_iso,
            row.compound_cid,
            row.taxon_qid,
            row.taxon_name,
            row.taxon_ncbi,
            row.taxon_ott,
            row.taxon_gbif,
            row.taxon_parent,
            row.taxon_rank,
            row.taxon_rank_label,
            row.reference_qid,
            row.reference_title,
            row.reference_doi,
            row.reference_pmid,
            row.reference_pmcid,
            row.reference_date,
            row.reference_journal,
            row.reference_journal_label,
        ];
        writer.write_record(record)?;
        count += 1;
    }
    writer.flush()?;
    Ok(count)
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

/// Number of sequential table requests in a full export.
pub const EXPORT_QUERY_COUNT: u64 = 4;

/// Run the full LOTUS export: fetch every table, index by QID, join, and
/// serialize `frozen.csv` plus `frozen_metadata.csv`.
///
/// `now` is the ISO-8601 stamp the host read; `year` is recorded in the
/// metadata and filters nothing. `endpoint` is the service to query (`QLever`
/// by default, overridable through its environment variable).
///
/// `on_progress` is called before each of the [`EXPORT_QUERY_COUNT`] table
/// requests with its name.
///
/// The fetched tables, their indexes and the finished CSV are all in memory
/// at once, so peak memory grows with the graph. The caller writes the two
/// returned strings to disk (native) or triggers a browser download (wasm);
/// this crate performs no file I/O, so it stays wasm-clean.
///
/// # Errors
/// Returns [`ExportError`] when a fetch, a parse, or a serialization fails.
pub async fn run_lotus_exporter<H: Http>(
    http: &H,
    endpoint: Endpoint,
    now: &str,
    year: u16,
    mut on_progress: impl FnMut(&'static str),
) -> Result<(String, String, ExportSummary), ExportError> {
    let triplet_bytes = fetch_export_bytes(
        http,
        &endpoint,
        COMPOUND_TAXON_REFERENCE_QUERY,
        "compound_taxon_reference",
        &mut on_progress,
    )
    .await?;
    let triplets = read_triplets(&triplet_bytes)?;
    let triplet_count = triplets.len();

    let compound_bytes = fetch_export_bytes(
        http,
        &endpoint,
        EXPORT_COMPOUNDS_QUERY,
        "compounds",
        &mut on_progress,
    )
    .await?;
    let compounds = build_compound_index(&compound_bytes)?;

    let taxon_bytes =
        fetch_export_bytes(http, &endpoint, EXPORT_TAXA_QUERY, "taxa", &mut on_progress).await?;
    let taxa = build_taxon_index(&taxon_bytes)?;

    let reference_bytes = fetch_export_bytes(
        http,
        &endpoint,
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
            value: endpoint.url().to_string(),
        },
        MetaLine {
            key: "date_created".into(),
            value: now.to_string(),
        },
        MetaLine {
            key: "year".into(),
            value: year.to_string(),
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
#[path = "pipeline/tests.rs"]
mod tests;
