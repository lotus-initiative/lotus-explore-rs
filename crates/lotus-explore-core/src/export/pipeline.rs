// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The LOTUS exporter pipeline: fetch Wikidata tables, join them, write the
//! export files.
//!
//! The pipeline runs the same queries as `lotus_exporter.py`, but through an
//! injected [`lotus_search::Http`] transport so a test can drive it against a scripted
//! double instead of the network. Every table is fetched once, indexed by QID,
//! and joined in memory before the output is streamed row by row.
//!
//! No clock: the caller supplies `now` (the ISO-8601 stamp) and `year`. No
//! UI framework: this crate draws nothing. The CLI and the app each supply
//! their own runtime and their own file writers.

use std::collections::HashMap;
use std::io::Write;

use lotus_query::{
    QLEVER_WIKIDATA, extract_qid, query_compound_cid, query_compound_smiles_can,
    query_compound_smiles_iso, query_compound_taxon_reference, query_reference_date,
    query_reference_doi, query_reference_journal, query_reference_pmcid, query_reference_pmid,
    query_reference_title, query_taxon_gbif, query_taxon_name, query_taxon_ncbi, query_taxon_ott,
    query_taxon_parent, query_taxon_rank,
};
use lotus_search::{Endpoint, FetchError, Http, ResponseFormat, execute_streaming};
use thiserror::Error;

/// One result row, as column name → cell value.
pub(crate) type Row = HashMap<String, String>;

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

/// The joined export row, in frozen.csv column order.
#[derive(Clone, Default)]
struct ExportRow {
    compound_qid: String,
    compound_inchikey: Option<String>,
    compound_smiles_can: Option<String>,
    compound_smiles_iso: Option<String>,
    compound_cid: Option<String>,
    taxon_qid: String,
    taxon_name: Option<String>,
    taxon_ncbi: Option<String>,
    taxon_ott: Option<String>,
    taxon_gbif: Option<String>,
    taxon_parent: Option<String>,
    taxon_rank: Option<String>,
    taxon_rank_label: Option<String>,
    reference_qid: String,
    reference_title: Option<String>,
    reference_doi: Option<String>,
    reference_pmid: Option<String>,
    reference_pmcid: Option<String>,
    reference_date: Option<String>,
    reference_journal: Option<String>,
    reference_journal_label: Option<String>,
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
fn read_csv_rows(bytes: &[u8]) -> Result<Vec<Row>, csv::Error> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(bytes);
    let headers: Vec<String> = reader.headers()?.iter().map(ToString::to_string).collect();
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        let mut row = HashMap::with_capacity(headers.len());
        for (i, header) in headers.iter().enumerate() {
            row.insert(
                header.clone(),
                record.get(i).map_or_else(String::new, str::to_string),
            );
        }
        rows.push(row);
    }
    Ok(rows)
}

/// POST one SPARQL query, read every chunk, and parse the CSV into rows.
pub(crate) async fn fetch_csv_rows<H: Http>(
    http: &H,
    endpoint: &Endpoint,
    query: &str,
    _table: &'static str,
) -> Result<Vec<Row>, ExportError> {
    let mut answer = execute_streaming(http, endpoint.clone(), query, ResponseFormat::Csv).await?;
    let mut body = String::new();
    loop {
        let chunk = answer.chunks.next_chunk().await?;
        match chunk {
            Some(bytes) => {
                let text = String::from_utf8_lossy(&bytes[..]);
                body.push_str(&text);
            }
            None => break,
        }
    }
    let rows = read_csv_rows(body.as_bytes()).map_err(ExportError::Csv)?;
    Ok(rows)
}

/// The value of one column, empty when absent.
fn cell(row: &Row, column: &str) -> String {
    row.get(column).cloned().unwrap_or_default()
}

/// Strip the Wikidata entity IRI to a bare QID, leaving a bare QID unchanged.
fn qid_of(value: &str) -> String {
    extract_qid(value).to_string()
}

/// Build `compound QID → CompoundProps` from the structure tables.
fn build_compound_index(
    smiles_can: &[Row],
    smiles_iso: &[Row],
    cids: &[Row],
) -> HashMap<String, CompoundProps> {
    let mut index: HashMap<String, CompoundProps> = HashMap::new();
    for row in smiles_can {
        let q = qid_of(&cell(row, "compound"));
        index.entry(q.clone()).or_default().smiles_can = Some(cell(row, "compound_smiles_can"));
    }
    for row in smiles_iso {
        let q = qid_of(&cell(row, "compound"));
        index.entry(q.clone()).or_default().smiles_iso = Some(cell(row, "compound_smiles_iso"));
    }
    for row in cids {
        let q = qid_of(&cell(row, "compound"));
        index.entry(q.clone()).or_default().cid = Some(cell(row, "compound_cid"));
    }
    index
}

/// Build `taxon QID → TaxonProps` from the taxon tables.
fn build_taxon_index(
    names: &[Row],
    ncbis: &[Row],
    otts: &[Row],
    gbifs: &[Row],
    parents: &[Row],
    ranks: &[Row],
) -> HashMap<String, TaxonProps> {
    let mut index: HashMap<String, TaxonProps> = HashMap::new();
    for row in names {
        let q = qid_of(&cell(row, "taxon"));
        index.entry(q.clone()).or_default().name = Some(cell(row, "taxon_name"));
    }
    for row in ncbis {
        let q = qid_of(&cell(row, "taxon"));
        index.entry(q.clone()).or_default().ncbi = Some(cell(row, "taxon_ncbi"));
    }
    for row in otts {
        let q = qid_of(&cell(row, "taxon"));
        index.entry(q.clone()).or_default().ott = Some(cell(row, "taxon_ott"));
    }
    for row in gbifs {
        let q = qid_of(&cell(row, "taxon"));
        index.entry(q.clone()).or_default().gbif = Some(cell(row, "taxon_gbif"));
    }
    for row in parents {
        let q = qid_of(&cell(row, "taxon"));
        index.entry(q.clone()).or_default().parent = Some(cell(row, "taxon_parent"));
    }
    for row in ranks {
        let q = qid_of(&cell(row, "taxon"));
        let e = index.entry(q.clone()).or_default();
        e.rank = Some(cell(row, "taxon_rank"));
        e.rank_label = Some(cell(row, "taxon_rank_label"));
    }
    index
}

/// Build `reference QID → ReferenceProps` from the reference tables.
fn build_reference_index(
    dois: &[Row],
    pmid_s: &[Row],
    pmc_ids: &[Row],
    titles: &[Row],
    dates: &[Row],
    journals: &[Row],
) -> HashMap<String, ReferenceProps> {
    let mut index: HashMap<String, ReferenceProps> = HashMap::new();
    for row in dois {
        let q = qid_of(&cell(row, "reference"));
        index.entry(q.clone()).or_default().doi = Some(cell(row, "doi"));
    }
    for row in pmid_s {
        let q = qid_of(&cell(row, "reference"));
        index.entry(q.clone()).or_default().pmid = Some(cell(row, "pmid"));
    }
    for row in pmc_ids {
        let q = qid_of(&cell(row, "reference"));
        index.entry(q.clone()).or_default().pmcid = Some(cell(row, "pmcid"));
    }
    for row in titles {
        let q = qid_of(&cell(row, "reference"));
        index.entry(q.clone()).or_default().title = Some(cell(row, "title"));
    }
    for row in dates {
        let q = qid_of(&cell(row, "reference"));
        index.entry(q.clone()).or_default().date = Some(cell(row, "date"));
    }
    for row in journals {
        let q = qid_of(&cell(row, "reference"));
        let e = index.entry(q.clone()).or_default();
        e.journal = Some(cell(row, "journal"));
        e.journal_label = Some(cell(row, "journal_label"));
    }
    index
}

/// Join one triplet with the indexes into one export row.
fn join_row(
    triplet: &Row,
    compounds: &HashMap<String, CompoundProps>,
    taxa: &HashMap<String, TaxonProps>,
    references: &HashMap<String, ReferenceProps>,
) -> ExportRow {
    let compound_qid = qid_of(&cell(triplet, "compound"));
    let taxon_qid = qid_of(&cell(triplet, "taxon"));
    let reference_qid = qid_of(&cell(triplet, "reference"));

    let compound = compounds.get(&compound_qid).cloned().unwrap_or_default();
    let taxon = taxa.get(&taxon_qid).cloned().unwrap_or_default();
    let reference = references.get(&reference_qid).cloned().unwrap_or_default();

    ExportRow {
        compound_qid,
        compound_inchikey: (!cell(triplet, "compound_inchikey").is_empty())
            .then_some(cell(triplet, "compound_inchikey")),
        compound_smiles_can: compound.smiles_can,
        compound_smiles_iso: compound.smiles_iso,
        compound_cid: compound.cid,
        taxon_qid,
        taxon_name: taxon.name,
        taxon_ncbi: taxon.ncbi,
        taxon_ott: taxon.ott,
        taxon_gbif: taxon.gbif,
        taxon_parent: taxon.parent,
        taxon_rank: taxon.rank,
        taxon_rank_label: taxon.rank_label,
        reference_qid,
        reference_title: reference.title,
        reference_doi: reference.doi,
        reference_pmid: reference.pmid,
        reference_pmcid: reference.pmcid,
        reference_date: reference.date,
        reference_journal: reference.journal,
        reference_journal_label: reference.journal_label,
    }
}

/// Join every triplet and stream the resulting rows to `out` as a CSV,
/// writing each row as soon as it is produced instead of buffering them all.
///
/// Returns the number of rows written.
fn write_joined_rows<W: Write>(
    triplets: &[Row],
    compounds: &HashMap<String, CompoundProps>,
    taxa: &HashMap<String, TaxonProps>,
    references: &HashMap<String, ReferenceProps>,
    out: &mut W,
) -> Result<usize, ExportError> {
    let mut writer = csv::Writer::from_writer(out);
    writer.write_record(FROZEN_COLUMNS)?;
    let mut count = 0;
    for triplet in triplets {
        let row = join_row(triplet, compounds, taxa, references);
        let record = [
            row.compound_qid.as_str(),
            row.compound_inchikey.as_deref().unwrap_or(""),
            row.compound_smiles_can.as_deref().unwrap_or(""),
            row.compound_smiles_iso.as_deref().unwrap_or(""),
            row.compound_cid.as_deref().unwrap_or(""),
            row.taxon_qid.as_str(),
            row.taxon_name.as_deref().unwrap_or(""),
            row.taxon_ncbi.as_deref().unwrap_or(""),
            row.taxon_ott.as_deref().unwrap_or(""),
            row.taxon_gbif.as_deref().unwrap_or(""),
            row.taxon_parent.as_deref().unwrap_or(""),
            row.taxon_rank.as_deref().unwrap_or(""),
            row.taxon_rank_label.as_deref().unwrap_or(""),
            row.reference_qid.as_str(),
            row.reference_title.as_deref().unwrap_or(""),
            row.reference_doi.as_deref().unwrap_or(""),
            row.reference_pmid.as_deref().unwrap_or(""),
            row.reference_pmcid.as_deref().unwrap_or(""),
            row.reference_date.as_deref().unwrap_or(""),
            row.reference_journal.as_deref().unwrap_or(""),
            row.reference_journal_label.as_deref().unwrap_or(""),
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

/// Run the full LOTUS export: fetch every table, index by QID, join, and
/// serialize `frozen.csv` plus `frozen_metadata.csv`.
///
/// `now` is the ISO-8601 stamp the host read; `year` decides whether the year
/// filter means anything. `endpoint` is the service to query (`QLever` by
/// default, overridable through its environment variable).
///
/// Each joined row is written to the CSV writer as soon as it is produced,
/// so a large export never holds all joined rows in memory at once. The
/// caller writes the two returned strings to disk (native) or triggers a
/// browser download (wasm); this crate performs no file I/O, so it stays
/// wasm-clean.
///
/// # Errors
/// Returns [`ExportError`] when a fetch, a parse, or a serialization fails. A
/// missing column is reported rather than emitting empty cells silently.
pub async fn run_lotus_exporter<H: Http>(
    http: &H,
    endpoint: Endpoint,
    now: &str,
    year: u16,
) -> Result<(String, String, ExportSummary), ExportError> {
    // 1. The core triplet: compound–taxon–reference with InChIKeys.
    let triplets = fetch_csv_rows(
        http,
        &endpoint,
        &query_compound_taxon_reference(),
        "compound_taxon_reference",
    )
    .await?;
    let triplet_count = triplets.len();

    // 2. Compound structure properties.
    let smiles_can = fetch_csv_rows(
        http,
        &endpoint,
        &query_compound_smiles_can(),
        "compound_smiles_can",
    )
    .await?;
    let smiles_iso = fetch_csv_rows(
        http,
        &endpoint,
        &query_compound_smiles_iso(),
        "compound_smiles_iso",
    )
    .await?;
    let cids = fetch_csv_rows(http, &endpoint, &query_compound_cid(), "compound_cid").await?;
    let compounds = build_compound_index(&smiles_can, &smiles_iso, &cids);

    // 3. Taxon properties.
    let names = fetch_csv_rows(http, &endpoint, &query_taxon_name(), "taxon_name").await?;
    let ncbis = fetch_csv_rows(http, &endpoint, &query_taxon_ncbi(), "taxon_ncbi").await?;
    let otts = fetch_csv_rows(http, &endpoint, &query_taxon_ott(), "taxon_ott").await?;
    let gbifs = fetch_csv_rows(http, &endpoint, &query_taxon_gbif(), "taxon_gbif").await?;
    let parents = fetch_csv_rows(http, &endpoint, &query_taxon_parent(), "taxon_parent").await?;
    let ranks = fetch_csv_rows(http, &endpoint, &query_taxon_rank(), "taxon_rank").await?;
    let taxa = build_taxon_index(&names, &ncbis, &otts, &gbifs, &parents, &ranks);

    // 4. Reference properties.
    let dois = fetch_csv_rows(http, &endpoint, &query_reference_doi(), "reference_doi").await?;
    let pmids = fetch_csv_rows(http, &endpoint, &query_reference_pmid(), "reference_pmid").await?;
    let pmc_ids =
        fetch_csv_rows(http, &endpoint, &query_reference_pmcid(), "reference_pmcid").await?;
    let titles =
        fetch_csv_rows(http, &endpoint, &query_reference_title(), "reference_title").await?;
    let dates = fetch_csv_rows(http, &endpoint, &query_reference_date(), "reference_date").await?;
    let journals = fetch_csv_rows(
        http,
        &endpoint,
        &query_reference_journal(),
        "reference_journal",
    )
    .await?;
    let references = build_reference_index(&dois, &pmids, &pmc_ids, &titles, &dates, &journals);

    // 5. Join every triplet and stream rows to the CSV writer as they are
    // produced, instead of collecting the whole table into a Vec first.
    let mut buffer = Vec::new();
    let rows_written = write_joined_rows(&triplets, &compounds, &taxa, &references, &mut buffer)?;

    let frozen_csv = String::from_utf8(buffer)
        .map_err(|e| ExportError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;

    // 6. Metadata.
    let meta_lines = vec![
        MetaLine {
            key: "endpoint".into(),
            value: QLEVER_WIKIDATA.to_string(),
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
