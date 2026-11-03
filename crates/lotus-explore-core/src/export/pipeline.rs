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
use std::sync::Arc;

use lotus_query::{
    COMPOUND_CID_QUERY, COMPOUND_SMILES_CAN_QUERY, COMPOUND_SMILES_ISO_QUERY,
    COMPOUND_TAXON_REFERENCE_QUERY, QLEVER_WIKIDATA, REFERENCE_DATE_QUERY, REFERENCE_DOI_QUERY,
    REFERENCE_JOURNAL_QUERY, REFERENCE_PMCID_QUERY, REFERENCE_PMID_QUERY, REFERENCE_TITLE_QUERY,
    TAXON_GBIF_QUERY, TAXON_NAME_QUERY, TAXON_NCBI_QUERY, TAXON_OTT_QUERY, TAXON_PARENT_QUERY,
    TAXON_RANK_QUERY, extract_qid,
};
use lotus_search::{Endpoint, FetchError, Http, ResponseFormat, execute_streaming};
use thiserror::Error;

/// One result row, as column name → cell value.
pub(crate) type Row = HashMap<Arc<str>, String>;

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

/// POST one SPARQL query, read every chunk, and parse the CSV into rows.
pub(crate) async fn fetch_csv_rows<H: Http>(
    http: &H,
    endpoint: &Endpoint,
    query: &str,
    _table: &'static str,
) -> Result<Vec<Row>, ExportError> {
    let mut answer = execute_streaming(http, endpoint.clone(), query, ResponseFormat::Csv).await?;
    let mut body = Vec::new();
    while let Some(bytes) = answer.chunks.next_chunk().await? {
        body.extend_from_slice(&bytes);
    }
    read_csv_rows(&body).map_err(ExportError::Csv)
}

/// The value of one column, empty when absent.
pub(crate) fn cell<'a>(row: &'a Row, column: &str) -> &'a str {
    row.get(column).map_or("", String::as_str)
}

/// The QID in one entity column, stripped of its Wikidata entity IRI prefix.
pub(crate) fn qid(row: &Row, column: &str) -> String {
    extract_qid(cell(row, column)).to_string()
}

/// Build `compound QID → CompoundProps` from the structure tables.
fn build_compound_index(
    smiles_can: &[Row],
    smiles_iso: &[Row],
    cids: &[Row],
) -> HashMap<String, CompoundProps> {
    let mut index: HashMap<String, CompoundProps> = HashMap::new();
    for row in smiles_can {
        index.entry(qid(row, "compound")).or_default().smiles_can =
            Some(cell(row, "compound_smiles_can").to_string());
    }
    for row in smiles_iso {
        index.entry(qid(row, "compound")).or_default().smiles_iso =
            Some(cell(row, "compound_smiles_iso").to_string());
    }
    for row in cids {
        index.entry(qid(row, "compound")).or_default().cid =
            Some(cell(row, "compound_cid").to_string());
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
        index.entry(qid(row, "taxon")).or_default().name =
            Some(cell(row, "taxon_name").to_string());
    }
    for row in ncbis {
        index.entry(qid(row, "taxon")).or_default().ncbi =
            Some(cell(row, "taxon_ncbi").to_string());
    }
    for row in otts {
        index.entry(qid(row, "taxon")).or_default().ott = Some(cell(row, "taxon_ott").to_string());
    }
    for row in gbifs {
        index.entry(qid(row, "taxon")).or_default().gbif =
            Some(cell(row, "taxon_gbif").to_string());
    }
    for row in parents {
        index.entry(qid(row, "taxon")).or_default().parent =
            Some(cell(row, "taxon_parent").to_string());
    }
    for row in ranks {
        let e = index.entry(qid(row, "taxon")).or_default();
        e.rank = Some(cell(row, "taxon_rank").to_string());
        e.rank_label = Some(cell(row, "taxon_rank_label").to_string());
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
        index.entry(qid(row, "reference")).or_default().doi = Some(cell(row, "doi").to_string());
    }
    for row in pmid_s {
        index.entry(qid(row, "reference")).or_default().pmid = Some(cell(row, "pmid").to_string());
    }
    for row in pmc_ids {
        index.entry(qid(row, "reference")).or_default().pmcid =
            Some(cell(row, "pmcid").to_string());
    }
    for row in titles {
        index.entry(qid(row, "reference")).or_default().title =
            Some(cell(row, "title").to_string());
    }
    for row in dates {
        index.entry(qid(row, "reference")).or_default().date = Some(cell(row, "date").to_string());
    }
    for row in journals {
        let e = index.entry(qid(row, "reference")).or_default();
        e.journal = Some(cell(row, "journal").to_string());
        e.journal_label = Some(cell(row, "journal_label").to_string());
    }
    index
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
        let compound_qid = extract_qid(cell(triplet, "compound"));
        let taxon_qid = extract_qid(cell(triplet, "taxon"));
        let reference_qid = extract_qid(cell(triplet, "reference"));

        let compound = compounds.get(compound_qid);
        let taxon = taxa.get(taxon_qid);
        let reference = references.get(reference_qid);

        let row = ExportRow {
            compound_qid,
            compound_inchikey: cell(triplet, "compound_inchikey"),
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
        COMPOUND_TAXON_REFERENCE_QUERY,
        "compound_taxon_reference",
    )
    .await?;
    let triplet_count = triplets.len();

    // 2. Compound structure properties.
    let smiles_can = fetch_csv_rows(
        http,
        &endpoint,
        COMPOUND_SMILES_CAN_QUERY,
        "compound_smiles_can",
    )
    .await?;
    let smiles_iso = fetch_csv_rows(
        http,
        &endpoint,
        COMPOUND_SMILES_ISO_QUERY,
        "compound_smiles_iso",
    )
    .await?;
    let cids = fetch_csv_rows(http, &endpoint, COMPOUND_CID_QUERY, "compound_cid").await?;
    let compounds = build_compound_index(&smiles_can, &smiles_iso, &cids);

    // 3. Taxon properties.
    let names = fetch_csv_rows(http, &endpoint, TAXON_NAME_QUERY, "taxon_name").await?;
    let ncbis = fetch_csv_rows(http, &endpoint, TAXON_NCBI_QUERY, "taxon_ncbi").await?;
    let otts = fetch_csv_rows(http, &endpoint, TAXON_OTT_QUERY, "taxon_ott").await?;
    let gbifs = fetch_csv_rows(http, &endpoint, TAXON_GBIF_QUERY, "taxon_gbif").await?;
    let parents = fetch_csv_rows(http, &endpoint, TAXON_PARENT_QUERY, "taxon_parent").await?;
    let ranks = fetch_csv_rows(http, &endpoint, TAXON_RANK_QUERY, "taxon_rank").await?;
    let taxa = build_taxon_index(&names, &ncbis, &otts, &gbifs, &parents, &ranks);

    // 4. Reference properties.
    let dois = fetch_csv_rows(http, &endpoint, REFERENCE_DOI_QUERY, "reference_doi").await?;
    let pmids = fetch_csv_rows(http, &endpoint, REFERENCE_PMID_QUERY, "reference_pmid").await?;
    let pmc_ids = fetch_csv_rows(http, &endpoint, REFERENCE_PMCID_QUERY, "reference_pmcid").await?;
    let titles = fetch_csv_rows(http, &endpoint, REFERENCE_TITLE_QUERY, "reference_title").await?;
    let dates = fetch_csv_rows(http, &endpoint, REFERENCE_DATE_QUERY, "reference_date").await?;
    let journals = fetch_csv_rows(
        http,
        &endpoint,
        REFERENCE_JOURNAL_QUERY,
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
